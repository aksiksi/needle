extern crate chromaprint_rust;
extern crate ffmpeg_next;
extern crate rayon;

use chromaprint_rust as chromaprint;

use std::path::Path;
use std::time::Duration;

#[cfg(feature = "rayon")]
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

// NOTE: Modifying this struct is a breaking change.
#[derive(Deserialize, Serialize)]
pub(crate) struct FrameHashes {
    pub(crate) hash_period: f32,
    pub(crate) hash_duration: f32,
    pub(crate) data: Vec<(u32, Duration)>,
    /// Size of the video, in bytes. This is used as a primitive hash
    /// to detect if the video file has changed since this data was
    /// generated.
    pub(crate) video_size: usize,
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

#[derive(Debug)]
pub struct Analyzer<'a, P: AsRef<Path> + Sync> {
    paths: Option<&'a [P]>,
    threaded_decoding: bool,
    force: bool,
}

impl Default for Analyzer<'_, &Path> {
    fn default() -> Self {
        Self {
            paths: Default::default(),
            threaded_decoding: Default::default(),
            force: Default::default(),
        }
    }
}

impl<'a, P: AsRef<Path> + 'a + Sync> Analyzer<'a, P> {
    pub fn from_files(paths: &'a [P], threaded_decoding: bool, force: bool) -> Self {
        Self {
            paths: Some(paths),
            threaded_decoding,
            force,
        }
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
    ) -> Result<Vec<(u32, Duration)>> {
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
                        let hash = crate::simhash::simhash32(raw_fingerprint.get());
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

        // Check if we've already analyzed this video.
        let video_size = std::fs::File::open(&path)?.metadata()?.len() as usize;
        if !self.force {
            if let Ok(f) = std::fs::File::open(&frame_hash_path) {
                let data: FrameHashes = bincode::deserialize_from(&f).unwrap();
                if data.video_size == video_size {
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
            video_size,
        };

        // Write results to disk.
        if persist {
            let mut f = std::fs::File::create(&frame_hash_path)?;
            bincode::serialize_into(&mut f, &frame_hashes)?;
        }

        Ok(frame_hashes)
    }

    pub fn run(&self, hash_period: f32, hash_duration: f32, persist: bool) -> Result<()> {
        if self.paths.is_none() {
            return Err(Error::AnalyzerMissingPaths.into());
        }

        #[cfg(feature = "rayon")]
        self.paths
            .unwrap()
            .par_iter()
            .map(|path| {
                self.run_single(path, hash_period, hash_duration, persist)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        #[cfg(not(feature = "rayon"))]
        self.paths
            .unwrap()
            .iter()
            .map(|path| {
                self.run_single(path, hash_period, hash_duration, persist)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        Ok(())
    }
}
