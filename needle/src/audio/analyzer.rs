extern crate chromaprint_rust;
extern crate ffmpeg_next;
#[cfg(feature = "rayon")]
extern crate rayon;

use chromaprint_rust as chromaprint;

use std::path::Path;
use std::time::Duration;

#[cfg(feature = "rayon")]
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Represents frame hash data for a single video file. This is the result of running
/// an [Analyzer] on a video file.
///
/// The struct contains the raw data as well as metadata about how the data was generated. The
/// original video size is included to allow for primitive duplicate checks when deciding whether
/// or not to skip analyzing a file.
#[derive(Debug, Deserialize, Serialize)]
pub struct FrameHashes {
    pub(crate) hash_period: f32,
    pub(crate) hash_duration: f32,
    pub(crate) data: Vec<u32>,
    pub(crate) md5: String,
}

impl FrameHashes {
    /// Load frame hashes from a path.
    fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::FrameHashDataNotFound(path.to_owned()).into());
        }
        let f = std::fs::File::open(path)?;
        Ok(bincode::deserialize_from(&f)?)
    }

    /// Load frame hash data using a video path.
    ///
    /// If `analyze` is set, the video is analyzed in-place. Otherwise, the frame data is
    /// loaded from alongside the video.
    pub fn from_video(video: impl AsRef<Path>, analyze: bool) -> Result<Self> {
        let video = video.as_ref();

        if !analyze {
            let path = video
                .to_owned()
                .with_extension(super::FRAME_HASH_DATA_FILE_EXT);
            Self::from_path(&path)
        } else {
            tracing::debug!(
                "starting in-place video analysis for {}...",
                video.display()
            );
            let analyzer = super::Analyzer::<&Path>::default().with_force(true);
            let frame_hashes = analyzer.run_single(
                video,
                super::DEFAULT_HASH_PERIOD,
                super::DEFAULT_HASH_DURATION,
                false,
            )?;
            tracing::debug!("completed in-place video analysis for {}", video.display());
            Ok(frame_hashes)
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }

    pub(crate) fn data(&self) -> &[u32] {
        &self.data
    }

    pub(crate) fn get(&self, index: usize) -> (u32, Duration) {
        let d = Duration::from_secs_f32(self.hash_duration + self.hash_period * index as f32);
        (self.data[index], d)
    }
}

/// Thin wrapper around the native `FFmpeg` audio decoder.
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

    fn from_stream(stream: ffmpeg_next::format::stream::Stream, threaded: bool) -> Result<Self> {
        let ctx = ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())?;
        let mut decoder = ctx.decoder();

        if threaded {
            decoder.set_threading(Self::build_threading_config());
        }

        let decoder = decoder.audio()?;

        Ok(Self { decoder })
    }

    fn send_packet(&mut self, packet: &ffmpeg_next::packet::Packet) -> Result<()> {
        Ok(self.decoder.send_packet(packet)?)
    }

    fn receive_frame(&mut self, frame: &mut ffmpeg_next::frame::Audio) -> Result<()> {
        Ok(self.decoder.receive_frame(frame)?)
    }
}

/// Analyzes one or more videos and converts them into [FrameHashes].
///
/// If `threaded_decoding` is set to `true`, video files will be distributed across multiple threads
/// based on the number of CPUs available. If `force` is set, any existing frame hash data on disk
/// will be **ignored**.
///
/// At a high-level, the analyzer does the following for a given video:
///
/// 1. Extracts the most suitable audio stream
/// 2. Decodes the audio frame-by-frame and resamples it for fingerprinting
/// 3. Builds a fingerprint (or hash) based on the provided `hash_duration`
/// 4. Returns a [FrameHashes] instance that contains the raw data and (optionally) writes it to disk
///    alongside the video
///
/// # Example
///
/// ```
/// use std::path::PathBuf;
/// use needle::audio::Analyzer;
/// # fn get_sample_paths() -> Vec<PathBuf> {
/// #     let resources = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
/// #     vec![
/// #         resources.join("sample-5s.mp4"),
/// #         resources.join("sample-shifted-4s.mp4"),
/// #     ]
/// # }
///
/// let video_paths: Vec<PathBuf> = get_sample_paths();
/// let analyzer = Analyzer::from_files(video_paths, false, false);
/// let frame_hashes = analyzer.run(1.0, 3.0, false).unwrap();
/// ```
#[derive(Debug)]
pub struct Analyzer<P: AsRef<Path>> {
    pub(crate) videos: Vec<P>,
    threaded_decoding: bool,
    force: bool,
}

impl<P: AsRef<Path>> Default for Analyzer<P> {
    fn default() -> Self {
        Self {
            videos: Default::default(),
            threaded_decoding: false,
            force: false,
        }
    }
}

impl<P: AsRef<Path>> Analyzer<P> {
    /// Constructs a new [Analyzer] from a list of video paths.
    pub fn from_files(videos: impl Into<Vec<P>>, threaded_decoding: bool, force: bool) -> Self {
        Self {
            videos: videos.into(),
            threaded_decoding,
            force,
        }
    }

    /// Returns the video paths used by this analyzer.
    pub fn videos(&self) -> &[P] {
        &self.videos
    }

    /// Returns a new [Analyzer] with `force` set to the provided value.
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// Returns a new [Analyzer] with `thread_decoding` set to the provided value.
    pub fn with_threaded_decoding(mut self, threaded_decoding: bool) -> Self {
        self.threaded_decoding = threaded_decoding;
        self
    }

    fn find_best_audio_stream(
        input: &ffmpeg_next::format::context::Input,
    ) -> ffmpeg_next::format::stream::Stream {
        input
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            .expect("unable to find an audio stream")
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
    ) -> Result<Vec<u32>> {
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
                    for (raw_fingerprint, _) in fingerprinter.feed(samples).unwrap() {
                        let hash = chromaprint::simhash::simhash32(raw_fingerprint.get());
                        hashes.push(hash);
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

    pub(crate) fn run_single(
        &self,
        path: impl AsRef<Path>,
        hash_period: f32,
        hash_duration: f32,
        persist: bool,
    ) -> Result<FrameHashes> {
        let span = tracing::span!(tracing::Level::TRACE, "run");
        let _enter = span.enter();

        let path = path.as_ref();
        let frame_hash_path = path.with_extension(super::FRAME_HASH_DATA_FILE_EXT);

        // Check if we've already analyzed this video by comparing MD5 hashes.
        let md5 = crate::util::compute_header_md5sum(path)?;
        if !self.force {
            if let Ok(f) = std::fs::File::open(&frame_hash_path) {
                let data: FrameHashes = bincode::deserialize_from(&f).unwrap();
                if data.md5 == md5 {
                    println!("Skipping analysis for {}...", path.display());
                    return Ok(data);
                }
            }
        }

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
            md5,
        };

        // Write results to disk.
        if persist {
            let mut f = std::fs::File::create(&frame_hash_path)?;
            bincode::serialize_into(&mut f, &frame_hashes)?;
        }

        Ok(frame_hashes)
    }
}

impl<P: AsRef<Path> + Sync> Analyzer<P> {
    /// Runs this analyzer.
    pub fn run(
        &self,
        hash_period: f32,
        hash_duration: f32,
        persist: bool,
        threading: bool,
    ) -> Result<Vec<FrameHashes>> {
        if self.videos.len() == 0 {
            return Err(Error::AnalyzerMissingPaths.into());
        }

        let mut data = Vec::new();

        if cfg!(feature = "rayon") && threading {
            #[cfg(feature = "rayon")]
            {
                data = self
                    .videos
                    .par_iter()
                    .map(|path| {
                        self.run_single(path, hash_period, hash_duration, persist)
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
            }
        } else {
            data.extend(self.videos.iter().map(|path| {
                self.run_single(path, hash_period, hash_duration, persist)
                    .unwrap()
            }));
        }

        Ok(data)
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;

    fn get_sample_paths() -> Vec<PathBuf> {
        let resources = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        vec![
            resources.join("sample-5s.mp4"),
            resources.join("sample-shifted-4s.mp4"),
        ]
    }

    #[test]
    fn test_analyzer() {
        let paths = get_sample_paths();
        let analyzer = Analyzer::from_files(paths.clone(), false, false);
        let data = analyzer.run(0.3, 3.0, false, false).unwrap();
        insta::assert_debug_snapshot!(data);
    }
}
