extern crate chromaprint_rust;
extern crate ffmpeg_next;

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::Display;
use std::path::Path;
use std::time::Duration;

use chromaprint_rust as chromaprint;
use serde::{Deserialize, Serialize};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use super::simhash::simhash32;
use super::util;
use super::Error;

pub const DEFAULT_HASH_MATCH_THRESHOLD: u16 = 15;
pub const DEFAULT_OPENING_SEARCH_PERCENTAGE: f32 = 0.33;
pub const DEFAULT_ENDING_SEARCH_PERCENTAGE: f32 = 0.25;
pub const DEFAULT_MIN_OPENING_DURATION: u16 = 20; // seconds
pub const DEFAULT_MIN_ENDING_DURATION: u16 = 20; // seconds
pub const DEFAULT_HASH_PERIOD: f32 = 0.3;
pub const DEFAULT_HASH_DURATION: f32 = 3.0;
pub const DEFAULT_OPENING_AND_ENDING_TIME_PADDING: f32 = 0.0; // seconds

const FRAME_HASH_DATA_FILE_EXT: &str = "needle.bin";
const SKIP_FILE_EXT: &str = "needle.skip.json";

// TODO: Include MD5 hash to avoid duplicating work.
#[derive(Deserialize, Serialize)]
pub struct FrameHashes {
    hash_period: f32,
    hash_duration: f32,
    data: Vec<(u32, Duration)>,
}

/// Wraps the `FFmpeg` audio decoder.
struct Decoder {
    decoder: ffmpeg_next::codec::decoder::Audio,
}

impl Decoder {
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
    src_match_hash: u32,
    dst_match_hash: u32,
    is_src_opening: bool,
    is_dst_opening: bool,
    src_hash_duration: Duration,
    dst_hash_duration: Duration,
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

#[derive(Copy, Clone, Debug, Default)]
struct SearchResult {
    opening: Option<(Duration, Duration)>,
    ending: Option<(Duration, Duration)>,
}

pub struct Analyzer<'a, P: AsRef<Path> + Sync> {
    paths: &'a [P],
    threaded_decoding: bool,
}

impl<'a, P: AsRef<Path> + 'a + Sync> Analyzer<'a, P> {
    pub fn new(paths: &'a [P], threaded_decoding: bool) -> anyhow::Result<Self> {
        Ok(Self {
            paths,
            threaded_decoding,
        })
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

    // Given an audio stream, computes the fingerprint for raw audio for the given duration.
    //
    // `count` can be used to limit the number of frames to process.
    fn process_frames(
        ctx: &mut ffmpeg_next::format::context::Input,
        stream_idx: usize,
        hash_duration: Duration,
        hash_period: Duration,
        threaded: bool,
    ) -> anyhow::Result<Vec<(u32, Duration)>> {
        let span = tracing::span!(tracing::Level::TRACE, "process_frames");
        let _enter = span.enter();

        let stream = ctx.stream(stream_idx).unwrap();
        let mut decoder = Decoder::from_stream(stream, threaded).unwrap();

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

        // Build an iterator over packets in the stream.
        let audio_packets = ctx
            .packets()
            .filter(|(s, _)| s.index() == stream_idx)
            .map(|(_, p)| p);

        for p in audio_packets {
            decoder.send_packet(&p).unwrap();
            while decoder.receive_frame(&mut frame).is_ok() {
                // Resample the frame to S16 stereo and return the frame delay.
                let mut delay = match resampler.run(&frame, &mut frame_resampled) {
                    Ok(v) => v,
                    // If resampling fails due to changed input, construct a new local resampler for this frame
                    // and swap out the global resampler.
                    Err(ffmpeg_next::Error::InputChanged) => {
                        let mut local_resampler = frame
                            .resampler(
                                ffmpeg_next::format::Sample::I16(
                                    ffmpeg_next::format::sample::Type::Packed,
                                ),
                                ffmpeg_next::ChannelLayout::STEREO,
                                target_sample_rate,
                            )
                            .unwrap();
                        let delay = local_resampler
                            .run(&frame, &mut frame_resampled)
                            .expect("failed to resample frame");

                        resampler = local_resampler;

                        delay
                    }
                    // We don't expect any other errors to occur.
                    Err(_) => panic!("unexpected error"),
                };

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
                    for (raw_fingerprint, ts) in fingerprinter.feed(samples).unwrap() {
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

    fn run_single(
        &self,
        path: impl AsRef<Path>,
        hash_period: f32,
        hash_duration: f32,
        persist: bool,
    ) -> anyhow::Result<FrameHashes> {
        let span = tracing::span!(tracing::Level::TRACE, "run");
        let _enter = span.enter();

        let path = path.as_ref();
        let mut ctx = ffmpeg_next::format::input(&path)?;
        let stream = Self::find_best_audio_stream(&ctx);
        let stream_idx = stream.index();
        let threaded = self.threaded_decoding;

        tracing::debug!("starting frame processing for {}", path.display());
        let frame_hashes = Self::process_frames(
            &mut ctx,
            stream_idx,
            Duration::from_secs_f32(hash_duration),
            Duration::from_secs_f32(hash_period),
            threaded,
        )?;
        tracing::debug!(
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
            let mut f = std::fs::File::create(path.with_extension(FRAME_HASH_DATA_FILE_EXT))?;
            bincode::serialize_into(&mut f, &frame_hashes)?;
        }

        Ok(frame_hashes)
    }

    pub fn run(
        &self,
        hash_period: f32,
        hash_duration: f32,
        persist: bool,
    ) -> anyhow::Result<Vec<FrameHashes>> {
        #[cfg(feature = "rayon")]
        let frame_hashes = self
            .paths
            .par_iter()
            .map(|path| {
                self.run_single(path, hash_period, hash_duration, persist)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        #[cfg(not(feature = "rayon"))]
        let frame_hashes = self
            .paths
            .iter()
            .map(|path| {
                self.run_single(path, hash_period, hash_duration, persist)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        Ok(frame_hashes)
    }
}

/// Compares two audio streams.
pub struct Comparator<'a, P: AsRef<Path>> {
    videos: &'a [P],
    hash_match_threshold: u32,
    opening_search_percentage: f32,
    ending_search_percentage: f32,
    min_opening_duration: Duration,
    min_ending_duration: Duration,
    time_padding: Duration,
}

impl<'a, P: AsRef<Path>> Comparator<'a, P> {
    pub fn from_files(
        videos: &'a [P],
        hash_match_threshold: u16,
        opening_search_percentage: f32,
        ending_search_percentage: f32,
        min_opening_duration: Duration,
        min_ending_duration: Duration,
        time_padding: Duration,
    ) -> Self {
        Self {
            videos,
            hash_match_threshold: hash_match_threshold as u32,
            opening_search_percentage,
            ending_search_percentage,
            min_opening_duration,
            min_ending_duration,
            time_padding,
        }
    }

    #[inline]
    fn compute_hash_for_match(hashes: &[(u32, Duration)], (start, end): (usize, usize)) -> u32 {
        let hashes: Vec<u32> = hashes.iter().map(|t| t.0).collect();
        crate::simhash::simhash32(&hashes[start..end + 1])
    }

    /// Runs a LCS (longest common substring) search between the two sets of hashes. This runs in
    /// O(n * m) time.
    fn longest_common_hash_match(
        &self,
        src: &[(u32, Duration)],
        dst: &[(u32, Duration)],
        src_max_opening_time: Duration,
        src_min_ending_time: Duration,
        dst_max_opening_time: Duration,
        dst_min_ending_time: Duration,
        src_hash_duration: Duration,
        dst_hash_duration: Duration,
        heap: &mut ComparatorHeap,
    ) {
        // Build the DP table of substrings.
        let mut table: Vec<Vec<usize>> = vec![vec![0; dst.len() + 1]; src.len() + 1];
        for i in 0..src.len() {
            for j in 0..dst.len() {
                let (src_hash, dst_hash) = (src[i].0, dst[j].0);
                if i == 0 || j == 0 {
                    table[i][j] = 0;
                } else if u32::count_ones(src_hash ^ dst_hash) <= self.hash_match_threshold {
                    table[i][j] = table[i - 1][j - 1] + 1;
                } else {
                    table[i][j] = 0;
                }
            }
        }

        // Walk through the table and find all valid substrings and insert them into
        // the heap.
        let mut i = src.len() - 1;
        while i > 0 {
            let mut j = dst.len() - 1;
            while j > 0 {
                // We need to find an entry where the current entry is non-zero
                // and the next entry is zero. This indicates that we are at the end
                // of a substring.
                if table[i][j] == 0
                    || (i < src.len() - 1 && j < dst.len() - 1 && table[i + 1][j + 1] != 0)
                {
                    j -= 1;
                    continue;
                }

                let (src_start_idx, src_end_idx) = (i - table[i][j], i);
                let (dst_start_idx, dst_end_idx) = (j - table[i][j], j);

                let (src_start, src_end) = (src[src_start_idx].1, src[src_end_idx].1);
                let (dst_start, dst_end) = (dst[dst_start_idx].1, dst[dst_end_idx].1);
                let (is_src_opening, is_src_ending) = (
                    src_end < src_max_opening_time,
                    src_start > src_min_ending_time,
                );
                let (is_dst_opening, is_dst_ending) = (
                    dst_end < dst_max_opening_time,
                    dst_start > dst_min_ending_time,
                );

                let is_valid = (is_src_opening
                    && (src_end - src_start) >= self.min_opening_duration)
                    || (is_src_ending && (src_end - src_start) >= self.min_ending_duration)
                    || (is_dst_opening && (dst_end - dst_start) >= self.min_opening_duration)
                    || (is_dst_ending && (dst_end - dst_start) >= self.min_ending_duration);

                if is_valid {
                    let src_match_hash =
                        Self::compute_hash_for_match(src, (src_start_idx, src_end_idx));
                    let dst_match_hash =
                        Self::compute_hash_for_match(dst, (dst_start_idx, dst_end_idx));

                    let entry = ComparatorHeapEntry {
                        score: table[i][j],
                        src_longest_run: (src_start, src_end),
                        dst_longest_run: (dst_start, dst_end),
                        src_match_hash,
                        dst_match_hash,
                        is_src_opening,
                        is_dst_opening,
                        src_hash_duration,
                        dst_hash_duration,
                    };

                    heap.push(entry);
                }

                j -= 1;
            }

            i -= 1;
        }
    }

    fn find_opening_and_ending(
        &self,
        src_hashes: &FrameHashes,
        dst_hashes: &FrameHashes,
    ) -> OpeningAndEndingInfo {
        let _g = tracing::span!(tracing::Level::TRACE, "find_opening_and_ending");

        let src_hash_data = &src_hashes.data;
        let dst_hash_data = &dst_hashes.data;
        let src_hash_duration = Duration::from_secs_f32(src_hashes.hash_duration);
        let dst_hash_duration = Duration::from_secs_f32(dst_hashes.hash_duration);

        let mut heap: ComparatorHeap =
            BinaryHeap::with_capacity(src_hash_data.len() + dst_hash_data.len());

        // Figure out the duration limits for opening and endings.
        let src_opening_search_idx =
            (src_hash_data.len() as f32 * self.opening_search_percentage) as usize;
        let src_ending_search_idx =
            (src_hash_data.len() as f32 * (1.0 - self.ending_search_percentage)) as usize;
        let dst_opening_search_idx =
            (dst_hash_data.len() as f32 * self.opening_search_percentage) as usize;
        let dst_ending_search_idx =
            (dst_hash_data.len() as f32 * (1.0 - self.ending_search_percentage)) as usize;
        let src_max_opening_time = src_hash_data[src_opening_search_idx].1;
        let src_min_ending_time = src_hash_data[src_ending_search_idx].1;
        let dst_max_opening_time = dst_hash_data[dst_opening_search_idx].1;
        let dst_min_ending_time = dst_hash_data[dst_ending_search_idx].1;

        self.longest_common_hash_match(
            src_hash_data,
            dst_hash_data,
            src_max_opening_time,
            src_min_ending_time,
            dst_max_opening_time,
            dst_min_ending_time,
            src_hash_duration,
            dst_hash_duration,
            &mut heap,
        );

        tracing::debug!(heap_size = heap.len(), "finished sliding window analysis");

        let (mut src_valid_openings, mut src_valid_endings) = (Vec::new(), Vec::new());
        let (mut dst_valid_openings, mut dst_valid_endings) = (Vec::new(), Vec::new());

        while let Some(entry) = heap.pop() {
            let (is_src_opening, is_dst_opening) = (entry.is_src_opening, entry.is_dst_opening);
            if is_src_opening {
                src_valid_openings.push(entry.clone());
            } else {
                src_valid_endings.push(entry.clone());
            }
            if is_dst_opening {
                dst_valid_openings.push(entry.clone());
            } else {
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

    fn check_for_skip_file(&self, path: &Path) -> bool {
        let skip_file = path.clone().with_extension(SKIP_FILE_EXT);
        skip_file.exists()
    }

    fn create_skip_file(&self, path: &Path, result: SearchResult) -> anyhow::Result<()> {
        let opening = result
            .opening
            .map(|(start, end)| (start.as_secs_f32(), end.as_secs_f32()));
        let ending = result
            .ending
            .map(|(start, end)| (start.as_secs_f32(), end.as_secs_f32()));
        if opening.is_none() && ending.is_none() {
            return Ok(());
        }

        let skip_file = path.clone().with_extension(SKIP_FILE_EXT);
        let mut skip_file = std::fs::File::create(skip_file)?;
        let data = serde_json::json!({"opening": opening, "ending": ending});
        serde_json::to_writer(&mut skip_file, &data)?;

        Ok(())
    }

    fn display_opening_ending_info(&self, path: &Path, result: SearchResult) {
        println!("\n{}\n", path.display());
        if let Some(opening) = result.opening {
            let (start, end) = opening;
            println!(
                "* Opening - {:?}-{:?}",
                util::format_time(start),
                util::format_time(end)
            );
        } else {
            println!("* Opening - N/A");
        }
        if let Some(ending) = result.ending {
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

    fn search(
        &self,
        src_path: &Path,
        dst_path: &Path,
        analyze: bool,
    ) -> anyhow::Result<OpeningAndEndingInfo> {
        tracing::debug!("started audio comparator");

        let (src_frame_hashes, dst_frame_hashes) = if !analyze {
            // Make sure frame data files exist for these videos.
            let src_data_path = src_path.clone().with_extension(FRAME_HASH_DATA_FILE_EXT);
            let dst_data_path = dst_path.clone().with_extension(FRAME_HASH_DATA_FILE_EXT);
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

            tracing::debug!("loaded hash frame data from disk");

            (src_frame_hashes, dst_frame_hashes)
        } else {
            // Otherwise, compute the hash data now by analyzing the video files.
            tracing::debug!("starting in-place video analysis...");

            let src_paths = vec![src_path];
            let dst_paths = vec![dst_path];
            let src_analyzer = Analyzer::new(&src_paths, false)?;
            let dst_analyzer = Analyzer::new(&dst_paths, false)?;
            let src_frame_hashes = src_analyzer
                .run(DEFAULT_HASH_PERIOD, DEFAULT_HASH_DURATION, false)?
                .into_iter()
                .next()
                .unwrap();
            let dst_frame_hashes = dst_analyzer
                .run(DEFAULT_HASH_PERIOD, DEFAULT_HASH_DURATION, false)?
                .into_iter()
                .next()
                .unwrap();
            tracing::debug!("completed analysis for src");

            (src_frame_hashes, dst_frame_hashes)
        };

        tracing::debug!("starting search for opening and ending");
        let info = self.find_opening_and_ending(&src_frame_hashes, &dst_frame_hashes);
        tracing::debug!("finished search for opening and ending");

        Ok(info)
    }

    /// Find the best opening and ending candidate across all provided matches.
    ///
    /// The idea is simple: keep track of the longest opening and ending detected among all of the matches
    /// and combine them to determine the best overall match.
    fn find_best_match(&self, matches: &[(&OpeningAndEndingInfo, bool)]) -> Option<SearchResult> {
        // TODO(aksiksi): Use the number of distinct matches along with duration. For example, it could be that the longest
        // opening was not actually the opening but instead a montage song found in one or two other episodes.
        if matches.len() == 0 {
            return None;
        }

        let mut result: SearchResult = Default::default();
        let mut best_opening_duration = Duration::ZERO;
        let mut best_ending_duration = Duration::ZERO;

        for (m, is_source) in matches {
            let opening;
            let ending;
            if *is_source {
                opening = m
                    .src_openings
                    .first()
                    .map(|e| (e.src_longest_run, e.src_hash_duration));
                ending = m
                    .src_endings
                    .first()
                    .map(|e| (e.src_longest_run, e.src_hash_duration));
            } else {
                opening = m
                    .dst_openings
                    .first()
                    .map(|e| (e.dst_longest_run, e.dst_hash_duration));
                ending = m
                    .dst_endings
                    .first()
                    .map(|e| (e.dst_longest_run, e.dst_hash_duration));
            }

            if let Some(((start, end), hash_duration)) = opening {
                let duration = end - start;
                if duration >= best_opening_duration {
                    result.opening = Some((
                        // Add a buffer between actual detected times and what we return to users.
                        start + self.time_padding,
                        // Adjust ending time using the configured hash duration.
                        end - self.time_padding - hash_duration,
                    ));
                    best_opening_duration = duration;
                }
            }
            if let Some(((start, end), hash_duration)) = ending {
                let duration = end - start;
                if duration >= best_ending_duration {
                    result.ending = Some((
                        // Add a buffer between actual detected times and what we return to users.
                        start + self.time_padding,
                        // Adjust ending time using the configured hash duration.
                        end - self.time_padding - hash_duration,
                    ));
                    best_ending_duration = duration;
                }
            }
        }

        Some(result)
    }
}

impl<'a, T: AsRef<Path> + std::marker::Sync> Comparator<'a, T> {
    pub fn run(&self, analyze: bool, display: bool, use_skip_files: bool) -> anyhow::Result<()> {
        // Build a list of video pairs for actual search. Pairs should only appear once.
        // Given N videos, this will result in: (N * (N-1)) / 2 pairs
        let mut pairs = Vec::new();
        let mut processed_videos = HashSet::new();
        for (i, v1) in self.videos.iter().enumerate() {
            let v1 = v1.as_ref();

            // Skip processing this video if it already has a skip file on disk.
            if use_skip_files && self.check_for_skip_file(v1) {
                // TODO(aksiksi): Check MD5 hash of the video against the skip file to handle
                // the case of a new file with the same name.
                println!("Skipping {} due to existing skip file...", v1.display());
                processed_videos.insert(v1);
                continue;
            }

            for (j, v2) in self.videos.iter().enumerate() {
                let v2 = v2.as_ref();
                if i == j || processed_videos.contains(v2) {
                    continue;
                }
                pairs.push((v1, v2));
            }
            processed_videos.insert(v1);
        }

        // Perform the search in parallel for all pairs.
        #[cfg(feature = "rayon")]
        let data = pairs
            .par_iter()
            .map(|(src_path, dst_path)| {
                (
                    *src_path,
                    *dst_path,
                    self.search(src_path, dst_path, analyze).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        #[cfg(not(feature = "rayon"))]
        let data = pairs
            .iter()
            .map(|(src_path, dst_path)| {
                (
                    *src_path,
                    *dst_path,
                    self.search(src_path, dst_path, analyze).unwrap(),
                )
            })
            .collect::<Vec<_>>();

        // This map tracks the generated info struct for each video path. A bool is included
        // to allow determining whether the path is a source (true) or dest (false) in the info
        // struct.
        let mut info_map: HashMap<&Path, Vec<(&OpeningAndEndingInfo, bool)>> = HashMap::new();
        for (src_path, dst_path, info) in &data {
            if let Some(v) = info_map.get_mut(*src_path) {
                v.push((info, true));
            } else {
                info_map.insert(*src_path, vec![(info, true)]);
            }
            if let Some(v) = info_map.get_mut(*dst_path) {
                v.push((info, false));
            } else {
                info_map.insert(*dst_path, vec![(info, false)]);
            }
        }

        // For each path, find the best opening and ending candidate among the list
        // of other videos. If required, display the result and write a skip file to disk.
        for (path, matches) in info_map {
            let result = self.find_best_match(&matches);
            if result.is_none() {
                println!("No opening or ending found for: {}", path.display());
                continue;
            }
            let result = result.unwrap();
            if display {
                self.display_opening_ending_info(path, result);
            }
            if use_skip_files {
                self.create_skip_file(path, result)?;
            }
        }

        Ok(())
    }
}
