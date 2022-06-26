extern crate chromaprint;
extern crate ffmpeg_next;

use std::collections::BinaryHeap;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::simhash::simhash32;
use super::util;
use super::Error;

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
    src_opening: Option<(Duration, Duration)>,
    src_ending: Option<(Duration, Duration)>,
    dst_opening: Option<(Duration, Duration)>,
    dst_ending: Option<(Duration, Duration)>,
}

impl OpeningAndEndingInfo {
    #[allow(unused)]
    fn validate_opening_and_ending_durations(
        &mut self,
        minimum_opening_duration: Duration,
        minimum_ending_duration: Duration,
    ) {
        self.src_opening = self
            .src_opening
            .filter(|(start, end)| *end - *start >= minimum_opening_duration);
        self.dst_opening = self
            .dst_opening
            .filter(|(start, end)| *end - *start >= minimum_opening_duration);
        self.src_ending = self
            .src_ending
            .filter(|(start, end)| *end - *start >= minimum_ending_duration);
        self.dst_ending = self
            .dst_ending
            .filter(|(start, end)| *end - *start >= minimum_ending_duration);
    }
}

/// Compares two audio streams.
pub struct AudioComparator {
    src_ctx: ffmpeg_next::format::context::Input,
    src_path: PathBuf,
    dst_ctx: ffmpeg_next::format::context::Input,
    dst_path: PathBuf,
    threading: bool,
}

impl AudioComparator {
    const CHROMAPRINT_MATCH_THRESHOLD: u32 = 10;

    pub fn new<P, Q>(src_path: P, dst_path: Q, threading: bool) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let src_ctx = ffmpeg_next::format::input(&src_path)?;
        let dst_ctx = ffmpeg_next::format::input(&dst_path)?;
        Ok(Self {
            src_ctx,
            dst_ctx,
            src_path: src_path.as_ref().to_path_buf(),
            dst_path: dst_path.as_ref().to_path_buf(),
            threading,
        })
    }

    fn src_stream(&self) -> ffmpeg_next::format::stream::Stream {
        self.src_ctx
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            .expect("unable to find an audio stream in source")
    }

    fn dst_stream(&self) -> ffmpeg_next::format::stream::Stream {
        self.dst_ctx
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            .expect("unable to find an audio stream in destination")
    }

    fn src_decoder(&mut self) -> anyhow::Result<AudioDecoder> {
        AudioDecoder::from_stream(self.src_stream(), self.threading)
    }

    fn dst_decoder(&mut self) -> anyhow::Result<AudioDecoder> {
        AudioDecoder::from_stream(self.dst_stream(), self.threading)
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

    // Decode and resample one packet in the stream to determinew what the current stream
    // delay is, if any.
    #[allow(unused)]
    fn find_initial_stream_delay(
        ctx: &mut ffmpeg_next::format::context::Input,
        stream_idx: usize,
        decoder: &mut AudioDecoder,
        resampler: &mut ffmpeg_next::software::resampling::Context,
    ) -> Option<Duration> {
        let first_packet = ctx
            .packets()
            .filter(|(s, _)| s.index() == stream_idx)
            .map(|(_, p)| p)
            .next();
        if first_packet.is_none() {
            return None;
        }

        // Decode the packet
        let mut frame = ffmpeg_next::frame::Audio::empty();
        let packet = first_packet.unwrap();
        decoder.send_packet(&packet).unwrap();
        if decoder.receive_frame(&mut frame).is_err() {
            return None;
        }

        // Resample the frame and return any delay
        let mut frame_resampled = ffmpeg_next::frame::Audio::empty();
        let delay = resampler.run(&frame, &mut frame_resampled).unwrap();
        delay.map(|d| Duration::from_millis(d.milliseconds as u64))
    }

    // Given an audio stream, computes the fingerprint for raw audio for the given duration.
    //
    // `count` can be used to limit the number of frames to process.
    fn process_frames(
        ctx: &mut ffmpeg_next::format::context::Input,
        decoder: &mut AudioDecoder,
        stream_idx: usize,
        hash_duration: Option<Duration>,
        duration: Option<Duration>,
        start_ts: Option<Duration>,
        write_samples: bool,
    ) -> (Vec<(u32, Duration)>, Vec<(Duration, Vec<u8>)>) {
        let span = tracing::span!(tracing::Level::TRACE, "process_frames");
        let _enter = span.enter();

        // If a start time is provided, seek to the correct place in the stream.
        if let Some(start_ts) = start_ts {
            Self::seek_to_timestamp(ctx, stream_idx, start_ts).unwrap();
        }
        // Compute the end time based on provided start time.
        let end_time = start_ts.and_then(|s| duration.map(|d| s + d));

        let mut hashes = Vec::new();
        let mut output_samples = Vec::new();
        let mut frame = ffmpeg_next::frame::Audio::empty();
        let mut frame_resampled = ffmpeg_next::frame::Audio::empty();

        // Setup the audio fingerprinter
        //
        // We set the hash resolution to 1/10th of the provided hash duration. Internally,
        // we will have 10 chromaprint instances.
        let n = 10;
        let hash_duration = hash_duration.unwrap_or(Duration::from_secs(1));
        let hash_resolution = hash_duration.div_f32(n as f32);
        let mut fingerprinter = chromaprint::DelayedFingerprinter::new(
            n,
            hash_duration,
            hash_resolution,
            None,
            2,
            None,
        );

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

        // TODO(aksiksi): Allow selection of stream.
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
            });

        for (p, _) in audio_packets {
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
                    let (_, samples, _) = unsafe { raw_samples.align_to() };

                    if write_samples {
                        output_samples.push((fingerprinter.clock(), raw_samples.to_vec()));
                    }

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

        (hashes, output_samples)
    }

    // Returns all packets for a given stream.
    #[allow(unused)]
    pub fn get_all_packets(
        ctx: &mut ffmpeg_next::format::context::Input,
        stream_idx: usize,
    ) -> Vec<ffmpeg_next::codec::packet::Packet> {
        ctx.packets()
            .filter(|(s, _)| s.index() == stream_idx)
            .map(|(_, p)| p)
            .collect()
    }

    // TODO(aksiksi): Document this.
    fn sliding_window_analyzer(
        src: &[(u32, Duration)],
        dst: &[(u32, Duration)],
        threshold: Option<u32>,
        heap: &mut ComparatorHeap,
        reverse: bool,
    ) {
        let threshold = threshold.unwrap_or(Self::CHROMAPRINT_MATCH_THRESHOLD);

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
        src_hashes: &[(u32, Duration)],
        dst_hashes: &[(u32, Duration)],
        opening_search_percentage: f32,
    ) -> Option<OpeningAndEndingInfo> {
        let _g = tracing::span!(tracing::Level::TRACE, "find_opening_and_ending");

        let mut heap: ComparatorHeap =
            BinaryHeap::with_capacity(src_hashes.len() + dst_hashes.len());

        let src_partition_idx = (src_hashes.len() as f32 * opening_search_percentage) as usize;
        let dst_partition_idx = (dst_hashes.len() as f32 * opening_search_percentage) as usize;

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
        //               [ --- src --- ]
        //                       [ --- dst --- ]
        //
        //               [ --- src --- ]
        //                             [ --- dst --- ]
        Self::sliding_window_analyzer(src_hashes, dst_hashes, None, &mut heap, false);
        Self::sliding_window_analyzer(dst_hashes, src_hashes, None, &mut heap, true);

        tracing::info!(heap_size = heap.len(), "finished sliding window analysis");

        let first = heap.pop();
        let second = heap.pop();

        // Next, we'll use the `opening_search_percentage` to determine which is an opening and which is an ending.
        let src_max_opening_time = src_hashes[src_partition_idx].1;
        let dst_max_opening_time = dst_hashes[dst_partition_idx].1;

        let info = match (first, second) {
            (Some(f), Some(s)) => {
                let (src_first_start, src_first_end) = f.src_longest_run;
                let (dst_first_start, dst_first_end) = f.dst_longest_run;
                let (src_second_start, src_second_end) = s.src_longest_run;
                let (dst_second_start, dst_second_end) = s.dst_longest_run;

                let src_opening = if src_first_end < src_max_opening_time {
                    Some((src_first_start, src_first_end))
                } else if src_second_end < src_max_opening_time {
                    Some((src_second_start, src_second_end))
                } else {
                    None
                };
                let src_ending = if src_first_end >= src_max_opening_time {
                    Some((src_first_start, src_first_end))
                } else if src_second_end >= src_max_opening_time {
                    Some((src_second_start, src_second_end))
                } else {
                    None
                };
                let dst_opening = if dst_first_end < dst_max_opening_time {
                    Some((dst_first_start, dst_first_end))
                } else if dst_second_end < dst_max_opening_time {
                    Some((dst_second_start, dst_second_end))
                } else {
                    None
                };
                let dst_ending = if dst_first_end >= dst_max_opening_time {
                    Some((dst_first_start, dst_first_end))
                } else if dst_second_end >= dst_max_opening_time {
                    Some((dst_second_start, dst_second_end))
                } else {
                    None
                };

                Some(OpeningAndEndingInfo {
                    src_opening,
                    src_ending,
                    dst_opening,
                    dst_ending,
                })
            }
            (Some(f), None) => {
                let (src_first_start, src_first_end) = f.src_longest_run;
                let (dst_first_start, dst_first_end) = f.dst_longest_run;

                let (src_opening, src_ending) = if src_first_end < src_max_opening_time {
                    (Some((src_first_start, src_first_end)), None)
                } else {
                    (None, Some((src_first_start, src_first_end)))
                };

                let (dst_opening, dst_ending) = if dst_first_end < dst_max_opening_time {
                    (Some((dst_first_start, dst_first_end)), None)
                } else {
                    (None, Some((dst_first_start, dst_first_end)))
                };

                Some(OpeningAndEndingInfo {
                    src_opening,
                    src_ending,
                    dst_opening,
                    dst_ending,
                })
            }
            (None, Some(s)) => {
                let (src_second_start, src_second_end) = s.src_longest_run;
                let (dst_second_start, dst_second_end) = s.dst_longest_run;

                let (src_opening, src_ending) = if src_second_end < src_max_opening_time {
                    (Some((src_second_start, src_second_end)), None)
                } else {
                    (None, Some((src_second_start, src_second_end)))
                };

                let (dst_opening, dst_ending) = if dst_second_end < dst_max_opening_time {
                    (Some((dst_second_start, dst_second_end)), None)
                } else {
                    (None, Some((dst_second_start, dst_second_end)))
                };

                Some(OpeningAndEndingInfo {
                    src_opening,
                    src_ending,
                    dst_opening,
                    dst_ending,
                })
            }

            (None, None) => None,
        };

        info
    }

    pub fn run(
        &mut self,
        write_result: bool,
        opening_search_percentage: f32,
        minimum_opening_duration: Duration,
        minimum_ending_duration: Duration,
    ) -> anyhow::Result<()> {
        let span = tracing::span!(tracing::Level::TRACE, "run");
        let _enter = span.enter();

        let (src_stream, dst_stream) = (self.src_stream(), self.dst_stream());
        let src_stream_idx = src_stream.index();
        let dst_stream_idx = dst_stream.index();
        let mut src_decoder = self.src_decoder()?;
        let mut dst_decoder = self.dst_decoder()?;

        // Compute hashes for both files in 3 second chunks.
        tracing::info!("starting frame processing for source");
        let (src_frame_hashes, src_samples) = Self::process_frames(
            &mut self.src_ctx,
            &mut src_decoder,
            src_stream_idx,
            Some(Duration::from_secs(3)),
            None,
            None,
            write_result,
        );
        tracing::info!(
            num_hashes = src_frame_hashes.len(),
            "completed frame processing for source"
        );

        tracing::info!("starting frame processing for dest");
        let (dst_frame_hashes, dst_samples) = Self::process_frames(
            &mut self.dst_ctx,
            &mut dst_decoder,
            dst_stream_idx,
            Some(Duration::from_secs(3)),
            None,
            None,
            write_result,
        );
        tracing::info!(
            num_hashes = dst_frame_hashes.len(),
            "completed frame processing for dest"
        );

        tracing::info!("starting search for opening and ending");
        let info = Self::find_opening_and_ending(
            &src_frame_hashes,
            &dst_frame_hashes,
            opening_search_percentage,
        );
        tracing::info!("finished search for opening and ending");

        if let Some(info) = info {
            self.display_opening_ending_info(
                &info,
                minimum_opening_duration,
                minimum_ending_duration,
                &src_samples,
                &dst_samples,
                write_result,
            );
        } else {
            eprintln!("No opening or ending found.");
        }

        Ok(())
    }

    fn display_opening_ending_info(
        &self,
        info: &OpeningAndEndingInfo,
        minimum_opening_duration: Duration,
        minimum_ending_duration: Duration,
        src_samples: &[(Duration, Vec<u8>)],
        dst_samples: &[(Duration, Vec<u8>)],
        write_result: bool,
    ) {
        println!("\nSource: {}\n", self.src_path.display());
        if let Some(opening) = info.src_opening {
            let (start, end) = opening;
            if end - start >= minimum_opening_duration {
                println!(
                    "* Opening - {:?}-{:?}",
                    util::format_time(start),
                    util::format_time(end)
                );
            } else {
                tracing::debug!(
                    "* Opening - {:?}-{:?} (too short)",
                    util::format_time(start),
                    util::format_time(end)
                );
                println!("* Opening - N/A");
            }
            if write_result {
                util::write_samples_in_range("opening_src.raw", opening, &src_samples);
            }
        } else {
            println!("* Opening - N/A");
        }
        if let Some(ending) = info.src_ending {
            let (start, end) = ending;
            if end - start >= minimum_ending_duration {
                println!(
                    "* Ending - {:?}-{:?}",
                    util::format_time(start),
                    util::format_time(end)
                );
            } else {
                tracing::debug!(
                    "* Ending - {:?}-{:?} (too short)",
                    util::format_time(start),
                    util::format_time(end)
                );
                println!("* Ending - N/A");
            }
            if write_result {
                util::write_samples_in_range("ending_src.raw", ending, &src_samples);
            }
        } else {
            println!("* Ending - N/A");
        }

        println!("\nDestination: {}\n", self.dst_path.display());
        if let Some(opening) = info.dst_opening {
            let (start, end) = opening;
            if end - start >= minimum_opening_duration {
                println!(
                    "* Opening - {:?}-{:?}",
                    util::format_time(start),
                    util::format_time(end)
                );
            } else {
                tracing::debug!(
                    "* Opening - {:?}-{:?} (too short)",
                    util::format_time(start),
                    util::format_time(end)
                );
                println!("* Opening: N/A");
            }
            if write_result {
                util::write_samples_in_range("opening_dst.raw", opening, &dst_samples);
            }
        } else {
            println!("* Opening: N/A");
        }
        if let Some(ending) = info.dst_ending {
            let (start, end) = ending;
            if end - start >= minimum_ending_duration {
                println!(
                    "* Ending - {:?}-{:?}",
                    util::format_time(start),
                    util::format_time(end)
                );
            } else {
                tracing::debug!(
                    "* Ending - {:?}-{:?} (too short)",
                    util::format_time(start),
                    util::format_time(end)
                );
                println!("* Ending - N/A");
            }
            if write_result {
                util::write_samples_in_range("ending_dst.raw", ending, &dst_samples);
            }
        } else {
            println!("* Ending - N/A");
        }
    }
}
