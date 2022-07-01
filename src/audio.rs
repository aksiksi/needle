extern crate chromaprint;
extern crate ffmpeg_next;

use std::collections::BinaryHeap;
use std::fmt::Display;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::simhash::simhash32;
use super::util;
use super::Error;

const DEFAULT_FRAME_HASH_DATA_EXT: &str = "needle.bin";

// Defaults
pub const DEFAULT_HASH_PERIOD: f32 = 0.3;
pub const DEFAULT_HASH_DURATION: f32 = 3.0;
pub const DEFAULT_HASH_MATCH_THRESHOLD: u16 = 15;
pub const DEFAULT_OPENING_SEARCH_PERCENTAGE: f32 = 0.75;
pub const DEFAULT_MIN_OPENING_DURATION: u16 = 20; // seconds
pub const DEFAULT_MIN_ENDING_DURATION: u16 = 20; // seconds

// TODO: Include MD5 hash to avoid duplicating work.
#[derive(Deserialize, Serialize)]
pub struct FrameHashes {
    hash_period: f32,
    hash_duration: f32,
    data: Vec<(u32, Duration)>,
}

/// Wraps the `ffmpeg` video decoder.
struct AudioDecoder {
    decoder: ffmpeg_next::codec::decoder::Audio,
}

impl AudioDecoder {
    fn build_threading_config() -> ffmpeg_next::codec::threading::Config {
        let mut config = ffmpeg_next::codec::threading::Config::default();
        config.count = std::thread::available_parallelism()
            .expect("unable to determine available parallelism")
            .get();
        config.kind = ffmpeg_next::codec::threading::Type::Frame;
        config
    }

    fn from_stream(
        stream: ffmpeg_next::format::stream::Stream,
        threaded: bool,
    ) -> anyhow::Result<Self> {
        let ctx = ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())?;
        let mut decoder = ctx.decoder();

        if threaded {
            decoder.set_threading(Self::build_threading_config());
        }

        let decoder = decoder.audio()?;

        Ok(Self { decoder })
    }

    fn send_packet(&mut self, packet: &ffmpeg_next::packet::Packet) -> anyhow::Result<()> {
        Ok(self.decoder.send_packet(packet)?)
    }

    fn receive_frame(&mut self, frame: &mut ffmpeg_next::frame::Audio) -> anyhow::Result<()> {
        Ok(self.decoder.receive_frame(frame)?)
    }
}

type ComparatorHeap = BinaryHeap<ComparatorHeapEntry>;

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct ComparatorHeapEntry {
    score: usize,
    src_longest_run: (Duration, Duration),
    dst_longest_run: (Duration, Duration),
}

impl Display for ComparatorHeapEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "score: {}, src_longest_run: {:?}, dst_longest_run: {:?}",
            self.score, self.src_longest_run, self.dst_longest_run
        )
    }
}

#[derive(Debug)]
struct OpeningAndEndingInfo {
    src_openings: Vec<ComparatorHeapEntry>,
    dst_openings: Vec<ComparatorHeapEntry>,
    src_endings: Vec<ComparatorHeapEntry>,
    dst_endings: Vec<ComparatorHeapEntry>,
}

pub struct AudioAnalyzer {
    path: PathBuf,
    threaded_decoding: bool,
}

impl AudioAnalyzer {
    pub fn new<P>(path: P, threaded_decoding: bool) -> anyhow::Result<Self>
    where
        P: Into<PathBuf>,
    {
        Ok(Self {
            path: path.into(),
            threaded_decoding,
        })
    }

    fn context(&self) -> anyhow::Result<ffmpeg_next::format::context::Input> {
        Ok(ffmpeg_next::format::input(&self.path)?)
    }

    fn find_best_audio_stream(
        input: &ffmpeg_next::format::context::Input,
    ) -> ffmpeg_next::format::stream::Stream {
        input
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            .expect("unable to find an audio stream")
    }

    // Returns the actual presentation timestamp for this frame (i.e., timebase agnostic).
    #[allow(unused)]
    fn frame_timestamp(
        ctx: &mut ffmpeg_next::format::context::Input,
        stream_idx: usize,
        frame: &ffmpeg_next::frame::Audio,
    ) -> Option<Duration> {
        ctx.stream(stream_idx)
            .map(|s| f64::from(s.time_base()))
            .and_then(|time_base| frame.timestamp().map(|t| t as f64 * time_base * 1000.0))
            .map(|ts| Duration::from_millis(ts as u64))
    }

    // Seeks the video stream to the given timestamp. Under the hood, this uses
    // the standard ffmpeg/libav function, `av_seek_frame`.
    fn seek_to_timestamp(
        ctx: &mut ffmpeg_next::format::context::Input,
        stream_idx: usize,
        timestamp: Duration,
    ) -> anyhow::Result<()> {
        let time_base: f64 = ctx.stream(stream_idx).unwrap().time_base().into();
        let duration = Duration::from_millis((ctx.duration() as f64 * time_base) as u64);

        // Ensure that the provided timestamp is valid (i.e., doesn't exceed duration of the video).
        anyhow::ensure!(
            timestamp < duration,
            Error::InvalidSeekTimestamp {
                requested: timestamp,
                duration,
            }
        );

        // Convert timestamp from ms to seconds, then divide by time_base to get the timestamp
        // in time_base units.
        let timestamp = (timestamp.as_millis() as f64 / time_base / 1000.0) as i64;
        ctx.seek_to_frame(
            stream_idx as i32,
            timestamp,
            ffmpeg_next::format::context::input::SeekFlags::empty(),
        )?;
        Ok(())
    }

    // Given an audio stream, computes the fingerprint for raw audio for the given duration.
    //
    // `count` can be used to limit the number of frames to process.
    fn process_frames(
        ctx: &mut ffmpeg_next::format::context::Input,
        stream_idx: usize,
        hash_duration: Duration,
        hash_period: Duration,
        duration: Option<Duration>,
        threaded: bool,
        // Debug options
        start_ts: Option<Duration>,
    ) -> anyhow::Result<Vec<(u32, Duration)>> {
        let span = tracing::span!(tracing::Level::TRACE, "process_frames");
        let _enter = span.enter();

        let stream = ctx.stream(stream_idx).unwrap();
        let mut decoder = AudioDecoder::from_stream(stream, threaded).unwrap();

        let mut hashes = Vec::new();
        let mut frame = ffmpeg_next::frame::Audio::empty();
        let mut frame_resampled = ffmpeg_next::frame::Audio::empty();

        // Setup the audio fingerprinter
        let n = f32::ceil(hash_duration.as_secs_f32() / hash_period.as_secs_f32()) as usize;
        let mut fingerprinter =
            chromaprint::DelayedFingerprinter::new(n, hash_duration, hash_period, None, 2, None);

        // Setup the audio resampler
        let target_sample_rate = fingerprinter.sample_rate();
        let mut resampler = decoder
            .decoder
            .resampler(
                ffmpeg_next::format::Sample::I16(ffmpeg_next::format::sample::Type::Packed),
                ffmpeg_next::ChannelLayout::STEREO,
                target_sample_rate,
            )
            .unwrap();

        // If a start time is provided, seek to the correct place in the stream.
        if let Some(start_ts) = start_ts {
            Self::seek_to_timestamp(ctx, stream_idx, start_ts).unwrap();
        }
        // Compute the end time based on provided start time.
        let end_time = start_ts.and_then(|s| duration.map(|d| s + d));

        // Build an iterator over packets in the stream.
        let audio_packets = ctx
            .packets()
            .filter(|(s, _)| s.index() == stream_idx)
            .map(|(s, p)| {
                let time_base = f64::from(s.time_base());
                let pts = p.pts().expect("unable to extract PTS from packet");
                let pts = Duration::from_millis((pts as f64 * time_base * 1000.0) as u64);
                (p, pts)
            })
            .take_while(|(_, pts)| {
                if let Some(end_time) = end_time {
                    *pts < end_time
                } else {
                    true
                }
            })
            .map(|(p, _)| p);

        for p in audio_packets {
            decoder.send_packet(&p).unwrap();
            while decoder.receive_frame(&mut frame).is_ok() {
                // Resample frame to S16 stereo and return the frame delay.
                let mut delay = resampler
                    .run(&frame, &mut frame_resampled)
                    .expect("frame resampling failed");

                loop {
                    // Obtain a slice of raw bytes in interleaved format.
                    // We have two channels, so the bytes look like this: c1, c1, c2, c2, c1, c1, c2, c2, ...
                    //
                    // Note that `data` is a fixed-size buffer. To get the _actual_ sample bytes, we need to use:
                    // a) sample count, b) channel count, and c) number of bytes per S16 sample.
                    let raw_samples = &frame_resampled.data(0)
                        [..frame_resampled.samples() * frame_resampled.channels() as usize * 2];

                    // Transmute the raw byte slice into a slice of i16 samples.
                    // This looks like: c1, c2, c1, c2, ...
                    //
                    // SAFETY: We know for a fact that the returned buffer contains i16 samples
                    // because we explicitly told the resampler to return S16 samples (see above).
                    let (_, samples, _) = unsafe { raw_samples.align_to() };

                    // Feed the i16 samples to Chromaprint. Since we are using the default sampling rate,
                    // Chromaprint will _not_ do any resampling internally.
                    for (raw_fingerprint, mut ts) in fingerprinter.feed(samples).unwrap() {
                        // Adjust the raw timestamp based on the actual stream start time. We need to do this because
                        // the fingerprinter starts its clock at 0 and is unaware of actual video time.
                        if let Some(start_ts) = start_ts {
                            ts += start_ts;
                        }
                        let hash = simhash32(raw_fingerprint.get());
                        hashes.push((hash, ts));
                    }

                    if delay.is_none() {
                        break;
                    } else {
                        delay = resampler.flush(&mut frame_resampled).unwrap();
                    }
                }
            }
        }

        Ok(hashes)
    }

    pub fn run(
        &self,
        hash_period: f32,
        hash_duration: f32,
        persist: bool,
    ) -> anyhow::Result<FrameHashes> {
        let span = tracing::span!(tracing::Level::TRACE, "run");
        let _enter = span.enter();

        let path = self.path.clone();
        let mut ctx = self.context()?;
        let stream = Self::find_best_audio_stream(&ctx);
        let stream_idx = stream.index();
        let threaded = self.threaded_decoding;

        tracing::info!("starting frame processing for {}", path.display());
        let frame_hashes = Self::process_frames(
            &mut ctx,
            stream_idx,
            Duration::from_secs_f32(hash_duration),
            Duration::from_secs_f32(hash_period),
            None,
            threaded,
            None,
        )?;
        tracing::info!(
            num_hashes = frame_hashes.len(),
            "completed frame processing for {}",
            path.display(),
        );

        let frame_hashes = FrameHashes {
            hash_period,
            hash_duration,
            data: frame_hashes,
        };

        // Write results to disk.
        if persist {
            let mut f = std::fs::File::create(path.with_extension(DEFAULT_FRAME_HASH_DATA_EXT))?;
            bincode::serialize_into(&mut f, &frame_hashes)?;
        }

        Ok(frame_hashes)
    }
}

/// Compares two audio streams.
pub struct AudioComparator {
    src_path: PathBuf,
    dst_path: PathBuf,
    hash_match_threshold: u16,
    opening_search_percentage: f32,
    minimum_opening_duration: Duration,
    minimum_ending_duration: Duration,
}

impl AudioComparator {
    pub fn from_files<P, Q>(
        src_path: P,
        dst_path: Q,
        hash_match_threshold: u16,
        opening_search_percentage: f32,
        minimum_opening_duration: Duration,
        minimum_ending_duration: Duration,
    ) -> anyhow::Result<Self>
    where
        P: Into<PathBuf>,
        Q: Into<PathBuf>,
    {
        let src_path = src_path.into();
        let dst_path = dst_path.into();

        Ok(Self {
            src_path,
            dst_path,
            hash_match_threshold,
            opening_search_percentage,
            minimum_opening_duration,
            minimum_ending_duration,
        })
    }

    // TODO(aksiksi): Document this.
    fn sliding_window_analyzer(
        &self,
        src: &[(u32, Duration)],
        dst: &[(u32, Duration)],
        heap: &mut ComparatorHeap,
        reverse: bool,
    ) {
        let threshold = self.hash_match_threshold as u32;
        let mut n = 1;

        while n <= dst.len() {
            let src_end = usize::min(n, src.len());
            let dst_start = dst.len() - n;
            let src_hashes = &src[..src_end];
            let dst_hashes = &dst[dst_start..];

            let mut in_run = false;
            let mut run_len = 0;
            let mut max_run_len = 0;
            let mut src_run_start_idx = 0;
            let mut dst_run_start_idx = 0;
            let mut src_longest_run = Default::default();
            let mut dst_longest_run = Default::default();

            for ((i, (src_hash, _)), (j, (dst_hash, _))) in src_hashes
                .iter()
                .enumerate()
                .zip(dst_hashes.iter().enumerate())
            {
                let d = u32::count_ones(src_hash ^ dst_hash);
                if d < threshold {
                    if in_run {
                        run_len += 1;
                    } else {
                        in_run = true;
                        run_len = 1;
                        src_run_start_idx = i;
                        dst_run_start_idx = j;
                    }
                } else if in_run {
                    in_run = false;
                    if run_len >= max_run_len {
                        max_run_len = run_len;
                        src_longest_run = (
                            src_hashes[src_run_start_idx].1,
                            src_hashes[src_run_start_idx + run_len].1,
                        );
                        dst_longest_run = (
                            dst_hashes[dst_run_start_idx].1,
                            dst_hashes[dst_run_start_idx + run_len].1,
                        );
                    }
                }
            }

            let score = max_run_len;

            let entry = if !reverse {
                ComparatorHeapEntry {
                    score,
                    src_longest_run,
                    dst_longest_run,
                }
            } else {
                ComparatorHeapEntry {
                    score,
                    dst_longest_run: src_longest_run,
                    src_longest_run: dst_longest_run,
                }
            };

            if score > 0 {
                heap.push(entry);
            }

            n += 1;
        }
    }

    fn find_opening_and_ending(
        &self,
        src_hashes: &[(u32, Duration)],
        dst_hashes: &[(u32, Duration)],
    ) -> OpeningAndEndingInfo {
        let _g = tracing::span!(tracing::Level::TRACE, "find_opening_and_ending");

        let mut heap: ComparatorHeap =
            BinaryHeap::with_capacity(src_hashes.len() + dst_hashes.len());

        let src_partition_idx = (src_hashes.len() as f32 * self.opening_search_percentage) as usize;
        let dst_partition_idx = (dst_hashes.len() as f32 * self.opening_search_percentage) as usize;

        // We do a single search for opening and ending in the source and dest.
        //
        // Intuition:
        //
        // (1)
        //               [ --- src --- ]
        // [ --- dst --- ]
        //
        //               [ --- src --- ]
        //       [ --- dst --- ]
        //
        //               [ --- src --- ]
        //               [ --- dst --- ]
        //
        // (2)
        //               [ --- dst --- ]
        // [ --- src --- ]
        //
        //               [ --- dst --- ]
        //       [ --- src --- ]
        //
        //               [ --- dst --- ]
        //               [ --- src --- ]
        //
        // When one is shorter than the other:
        //
        // (1)
        //               [ --- src --- ]
        //     [ - dst - ]
        //
        //               [ --- src --- ]
        //          [ - dst - ]
        //
        //               [ --- src --- ]
        //               [ - dst - ]
        //
        // (2)
        //               [ - dst - ]
        // [ --- src --- ]
        //
        //               [ - dst - ]
        //     [ --- src --- ]
        //
        //               [ - dst - ]
        //           [ --- src --- ]
        //
        //               [ - dst - ]
        //               [ --- src --- ]
        self.sliding_window_analyzer(src_hashes, dst_hashes, &mut heap, false);
        self.sliding_window_analyzer(dst_hashes, src_hashes, &mut heap, true);

        tracing::info!(heap_size = heap.len(), "finished sliding window analysis");

        // Next, we'll use the `opening_search_percentage` to determine which is an opening and which is an ending.
        let src_max_opening_time = src_hashes[src_partition_idx].1;
        let dst_max_opening_time = dst_hashes[dst_partition_idx].1;

        let (mut src_valid_openings, mut src_valid_endings) = (Vec::new(), Vec::new());
        let (mut dst_valid_openings, mut dst_valid_endings) = (Vec::new(), Vec::new());

        while let Some(entry) = heap.pop() {
            let (src_start, src_end) = entry.src_longest_run;
            let (dst_start, dst_end) = entry.dst_longest_run;
            let (src_duration, dst_duration) = (src_end - src_start, dst_end - dst_start);

            let valid_duration = src_duration >= self.minimum_opening_duration
                || src_duration >= self.minimum_ending_duration
                || dst_duration >= self.minimum_opening_duration
                || dst_duration >= self.minimum_ending_duration;
            if !valid_duration {
                break;
            }

            if src_duration >= self.minimum_opening_duration && src_end <= src_max_opening_time {
                src_valid_openings.push(entry.clone());
            } else if src_duration >= self.minimum_ending_duration
                && src_start >= src_max_opening_time
            {
                src_valid_endings.push(entry.clone());
            }

            if dst_duration >= self.minimum_opening_duration && dst_end <= dst_max_opening_time {
                dst_valid_openings.push(entry.clone());
            } else if dst_duration >= self.minimum_ending_duration
                && dst_start >= dst_max_opening_time
            {
                dst_valid_endings.push(entry.clone());
            }
        }

        OpeningAndEndingInfo {
            src_openings: src_valid_openings,
            dst_openings: dst_valid_openings,
            src_endings: src_valid_endings,
            dst_endings: dst_valid_endings,
        }
    }

    pub fn run(&self, analyze: bool) -> anyhow::Result<()> {
        tracing::info!("started audio comparator");

        let (src_frame_hashes, dst_frame_hashes) = if !analyze {
            // Make sure frame data files exist for these videos.
            let src_data_path = self
                .src_path
                .clone()
                .with_extension(DEFAULT_FRAME_HASH_DATA_EXT);
            let dst_data_path = self
                .dst_path
                .clone()
                .with_extension(DEFAULT_FRAME_HASH_DATA_EXT);
            if !src_data_path.exists() {
                return Err(Error::FrameHashDataNotFound(src_data_path).into());
            }
            if !dst_data_path.exists() {
                return Err(Error::FrameHashDataNotFound(dst_data_path).into());
            }

            // Load frame hash data from disk.
            let src_file = std::fs::File::open(&src_data_path)?;
            let dst_file = std::fs::File::open(&dst_data_path)?;
            let src_frame_hashes: FrameHashes = bincode::deserialize_from(&src_file).expect(
                &format!("invalid frame hash data file: {}", src_data_path.display()),
            );
            let dst_frame_hashes: FrameHashes = bincode::deserialize_from(&dst_file).expect(
                &format!("invalid frame hash data file: {}", dst_data_path.display()),
            );

            tracing::info!("loaded hash frame data from disk");

            (src_frame_hashes, dst_frame_hashes)
        } else {
            // Otherwise, compute the hash data now by analyzing the video files.
            tracing::info!("starting in-place video analysis...");

            let src_analyzer = AudioAnalyzer::new(self.src_path.clone(), false)?;
            let dst_analyzer = AudioAnalyzer::new(self.dst_path.clone(), false)?;

            let dst_handle = std::thread::spawn(move || {
                let result = dst_analyzer.run(DEFAULT_HASH_PERIOD, DEFAULT_HASH_DURATION, false);
                tracing::info!("completed analysis for dst");
                result
            });

            let src_frame_hashes = src_analyzer.run(DEFAULT_HASH_PERIOD, DEFAULT_HASH_DURATION, false)?;
            tracing::info!("completed analysis for src");
            let dst_frame_hashes = dst_handle.join().unwrap()?;

            (src_frame_hashes, dst_frame_hashes)
        };

        tracing::info!("starting search for opening and ending");
        let info =
            self.find_opening_and_ending(&src_frame_hashes.data, &dst_frame_hashes.data);
        tracing::info!("finished search for opening and ending");

        self.display_opening_ending_info(&info);

        Ok(())
    }

    fn display_opening_ending_info(&self, info: &OpeningAndEndingInfo) {
        println!("\nSource: {}\n", self.src_path.display());

        if let Some(opening) = info.src_openings.first() {
            let opening = opening.src_longest_run;
            let (start, end) = opening;
            println!(
                "* Opening - {:?}-{:?}",
                util::format_time(start),
                util::format_time(end)
            );
        } else {
            println!("* Opening - N/A");
        }
        if let Some(ending) = info.src_endings.first() {
            let ending = ending.src_longest_run;
            let (start, end) = ending;
            println!(
                "* Ending - {:?}-{:?}",
                util::format_time(start),
                util::format_time(end)
            );
        } else {
            println!("* Ending - N/A");
        }

        println!("\nDestination: {}\n", self.dst_path.display());

        if let Some(opening) = info.dst_openings.first() {
            let opening = opening.dst_longest_run;
            let (start, end) = opening;
            println!(
                "* Opening - {:?}-{:?}",
                util::format_time(start),
                util::format_time(end)
            );
        } else {
            println!("* Opening - N/A");
        }
        if let Some(ending) = info.dst_endings.first() {
            let ending = ending.dst_longest_run;
            let (start, end) = ending;
            println!(
                "* Ending - {:?}-{:?}",
                util::format_time(start),
                util::format_time(end)
            );
        } else {
            println!("* Ending - N/A");
        }
    }
}
