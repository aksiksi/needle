extern crate chromaprint;
extern crate ffmpeg_next;

use std::path::Path;
use std::time::Duration;

use super::Error;

/// Wraps the `ffmpeg` video decoder.
struct AudioDecoder {
    decoder: ffmpeg_next::codec::decoder::Audio,
}

impl AudioDecoder {
    fn build_threading_config() -> ffmpeg_next::codec::threading::Config {
        let mut config = ffmpeg_next::codec::threading::Config::default();
        config.count = num_cpus::get();
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

    fn channels(&self) -> u16 {
        self.decoder.channels()
    }

    fn bit_rate(&self) -> usize {
        self.decoder.bit_rate()
    }

    fn sample_rate(&self) -> u32 {
        self.decoder.rate()
    }

    fn format(&self) -> ffmpeg_next::format::Sample {
        self.decoder.format()
    }

    fn channel_layout(&self) -> ffmpeg_next::ChannelLayout {
        self.decoder.channel_layout()
    }

    fn send_packet(&mut self, packet: &ffmpeg_next::packet::Packet) -> anyhow::Result<()> {
        Ok(self.decoder.send_packet(packet)?)
    }

    fn receive_frame(&mut self, frame: &mut ffmpeg_next::frame::Audio) -> anyhow::Result<()> {
        Ok(self.decoder.receive_frame(frame)?)
    }
}

/// Compares two audio streams.
pub struct AudioComparator {
    src_ctx: ffmpeg_next::format::context::Input,
    dst_ctx: ffmpeg_next::format::context::Input,
    src_hash_ctx: chromaprint::Context,
    dst_hash_ctx: chromaprint::Context,
}

impl AudioComparator {
    const FRAME_HASH_MATCH_THRESHOLD: u32 = 10;

    pub fn new<P, Q>(src_path: P, dst_path: Q) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let src_ctx = ffmpeg_next::format::input(&src_path)?;
        let dst_ctx = ffmpeg_next::format::input(&dst_path)?;
        let src_hash_ctx = chromaprint::Context::default();
        let dst_hash_ctx = chromaprint::Context::default();
        Ok(Self {
            src_ctx,
            dst_ctx,
            src_hash_ctx,
            dst_hash_ctx,
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
        AudioDecoder::from_stream(self.src_stream(), false)
    }

    fn dst_decoder(&mut self) -> anyhow::Result<AudioDecoder> {
        AudioDecoder::from_stream(self.dst_stream(), false)
    }

    // Returns the blockhash of the given frame.
    #[inline(always)]
    fn hash_frame(f: &ffmpeg_next::frame::Audio) -> anyhow::Result<u32> {
        assert!(
            f.format()
                == ffmpeg_next::format::Sample::I16(ffmpeg_next::format::sample::Type::Packed)
        );
        let mut ctx = chromaprint::Context::default();
        ctx.start(f.rate(), f.channels());
        ctx.feed(f.plane(0))?;
        ctx.finish()?;
        Ok(ctx.get_fingerprint_hash()?.get())
    }

    // Compares two frames by computing their blockhashes and returns the
    // difference (Hamming distance).
    #[inline(always)]
    fn compare_two_frames(
        f1: &ffmpeg_next::frame::Audio,
        f2: &ffmpeg_next::frame::Audio,
    ) -> anyhow::Result<u32> {
        let d1 = Self::hash_frame(f1)?;
        let d2 = Self::hash_frame(f2)?;
        Ok(u32::count_ones(d1 ^ d2))
    }

    // Returns the actual presentation timestamp for this frame (i.e., timebase agnostic).
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
    // `count` can be used to limit the number of frames to process. To sample fewer frames,
    // use the `skip_by` option. For example, if `skip_by` is set to 5, one in every 5 frames
    // will be processed.
    fn process_frames(
        ctx: &mut ffmpeg_next::format::context::Input,
        decoder: &mut AudioDecoder,
        stream_idx: usize,
        hash_ctx: &mut chromaprint::Context,
        hash_duration: Option<Duration>,
        count: Option<usize>,
        skip_by: Option<usize>,
    ) -> Vec<(u32, Duration)> {
        let _g = tracing::span!(tracing::Level::TRACE, "process_frames", count);

        let skip_by = skip_by.unwrap_or(1);
        let mut output = Vec::new();
        let mut frame = ffmpeg_next::frame::Audio::empty();
        let mut resampled = ffmpeg_next::frame::Audio::empty();

        let hash_duration = hash_duration.unwrap_or(Duration::from_secs(1));
        let mut last_hash_duration = None;
        hash_ctx
            .start(decoder.sample_rate(), 2)
            .unwrap();

        let mut resampler = decoder
            .decoder
            .resampler(
                ffmpeg_next::format::Sample::I16(ffmpeg_next::format::sample::Type::Packed),
                ffmpeg_next::ChannelLayout::STEREO,
                decoder.sample_rate(),
            )
            .unwrap();

        ctx.packets()
            .filter(|(s, _)| s.index() == stream_idx)
            .take(count.unwrap_or(usize::MAX))
            .enumerate()
            .map(|(i, (s, mut p))| {
                if i % skip_by != 0 {
                    p.set_flags(ffmpeg_next::codec::packet::Flags::DISCARD);
                }
                let time_base = f64::from(s.time_base());
                let pts = p.pts().unwrap();
                let ts = Duration::from_millis((pts as f64 * time_base * 1000.0) as u64);

                (s, p, ts)
            })
            .for_each(|(s, p, ts)| {
                if last_hash_duration.is_none() {
                    last_hash_duration = Some(ts);
                }

                decoder.send_packet(&p).unwrap();
                while decoder.receive_frame(&mut frame).is_ok() {
                    // Resample frame to S16 stereo.
                    resampler.run(&frame, &mut resampled).unwrap();

                    hash_ctx.feed(resampled.plane(0)).unwrap();
                    let last = last_hash_duration.unwrap();

                    // If the duration has passed, generate a fingerprint.
                    if ts >= last && (ts - last) >= hash_duration {
                        hash_ctx.finish().unwrap();
                        let hash = hash_ctx.get_fingerprint_hash().unwrap().get();
                        output.push((hash, ts));
                        hash_ctx.clear_fingerprint().unwrap();
                        hash_ctx
                            .start(resampled.rate(), 2)
                            .unwrap();
                        last_hash_duration = Some(ts);
                    }
                }
            });

        // We're always in start state.
        hash_ctx.finish().unwrap();

        output
    }

    // Returns all packets for a given stream.
    fn get_all_packets(
        ctx: &mut ffmpeg_next::format::context::Input,
        stream_idx: usize,
    ) -> Vec<ffmpeg_next::codec::packet::Packet> {
        ctx.packets()
            .filter(|(s, _)| s.index() == stream_idx)
            .map(|(_, p)| p)
            .collect()
    }

    pub fn compare(&mut self, _count: usize) -> anyhow::Result<()> {
        let (src_stream, dst_stream) = (self.src_stream(), self.dst_stream());
        let src_stream_idx = src_stream.index();
        let dst_stream_idx = dst_stream.index();
        let mut src_decoder = self.src_decoder()?;
        let mut dst_decoder = self.dst_decoder()?;

        let packets = Self::get_all_packets(&mut self.src_ctx, src_stream_idx);
        tracing::info!(num_packets = packets.len());

        Self::seek_to_timestamp(&mut self.src_ctx, src_stream_idx, Duration::from_secs(208))?;
        Self::seek_to_timestamp(&mut self.dst_ctx, dst_stream_idx, Duration::from_secs(174))?;

        let src_frame_hashes = Self::process_frames(
            &mut self.src_ctx,
            &mut src_decoder,
            src_stream_idx,
            &mut self.src_hash_ctx,
            Some(Duration::from_millis(1000)),
            None,
            Some(5),
        );
        let dst_frame_hashes = Self::process_frames(
            &mut self.dst_ctx,
            &mut dst_decoder,
            dst_stream_idx,
            &mut self.dst_hash_ctx,
            Some(Duration::from_millis(1000)),
            None,
            Some(5),
        );
        for ((h1, t1), (h2, t2)) in src_frame_hashes.iter().zip(dst_frame_hashes.iter().skip(1)) {
            tracing::info!(
                t1 = t1.as_millis() as u64,
                t2 = t2.as_millis() as u64,
                h1 = h1,
                h2 = h2,
                similarity = u32::count_ones(h1 ^ h2),
            );
        }

        Ok(())
    }
}
