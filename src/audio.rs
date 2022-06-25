extern crate chromaprint;
extern crate ffmpeg_next;

use std::collections::BinaryHeap;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use super::simhash::simhash32;
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

    fn send_packet(&mut self, packet: &ffmpeg_next::packet::Packet) -> anyhow::Result<()> {
        Ok(self.decoder.send_packet(packet)?)
    }

    fn receive_frame(&mut self, frame: &mut ffmpeg_next::frame::Audio) -> anyhow::Result<()> {
        Ok(self.decoder.receive_frame(frame)?)
    }
}

type ComparatorHeap<'a> = BinaryHeap<ComparatorHeapEntry<'a>>;

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord)]
struct ComparatorHeapEntry<'a> {
    // priority: number of hits * max run length
    priority: usize,
    src_longest_run: &'a [(u32, Duration)],
    dst_longest_run: &'a [(u32, Duration)],
    hash_data: Vec<(Duration, Duration, u32)>,
}

impl<'a> Display for ComparatorHeapEntry<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "priority: {}, src_longest_run: {:?}, dst_longest_run: {:?}",
            self.priority, self.src_longest_run, self.dst_longest_run
        )
    }
}

/// Compares two audio streams.
pub struct AudioComparator {
    src_ctx: ffmpeg_next::format::context::Input,
    dst_ctx: ffmpeg_next::format::context::Input,
}

impl AudioComparator {
    const CHROMAPRINT_MATCH_THRESHOLD: u32 = 10;

    pub fn new<P, Q>(src_path: P, dst_path: Q) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let src_ctx = ffmpeg_next::format::input(&src_path)?;
        let dst_ctx = ffmpeg_next::format::input(&dst_path)?;
        Ok(Self { src_ctx, dst_ctx })
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
        output: Option<impl AsRef<Path>>,
    ) -> Vec<(u32, Duration)> {
        let _g = tracing::span!(tracing::Level::TRACE, "process_frames");

        let mut f = output.map(|p| std::fs::File::create(p).unwrap());

        // If a start time is provided, seek to the correct place in the stream.
        if let Some(start_ts) = start_ts {
            Self::seek_to_timestamp(ctx, stream_idx, start_ts).unwrap();
        }
        // Compute the end time based on provided start time.
        let end_time = start_ts.and_then(|s| duration.map(|d| s + d));

        let mut hashes = Vec::new();
        let mut frame = ffmpeg_next::frame::Audio::empty();
        let mut frame_resampled = ffmpeg_next::frame::Audio::empty();

        // Setup the audio fingerprinter
        //
        // We set the hash resolution to 1/10th of the provided hash duration. Internally,
        // we will have 10 chromaprint instances.
        let n = 10;
        let hash_duration = hash_duration.unwrap_or(Duration::from_secs(1));
        let hash_resolution = hash_duration.div_f32(n as f32);
        let mut fingerprinter =
            chromaprint::DelayedFingerprinter::new(n, hash_duration, hash_resolution, None, 2);

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
                let delay = resampler
                    .run(&frame, &mut frame_resampled)
                    .expect("frame resampling failed")
                    .map(|d| Duration::from_millis(d.milliseconds as u64));

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

                if let Some(f) = &mut f {
                    f.write(raw_samples).unwrap();
                }

                // Feed the i16 samples to Chromaprint. Since we are using the default sampling rate,
                // Chromaprint will _not_ do any resampling internally.
                for (raw_fingerprint, mut raw_ts) in fingerprinter.feed(samples).unwrap() {
                    // Adjust the raw timestamp based on the actual stream start time. We need to do this because
                    // the fingerprinter starts its clock at 0 and is unaware of actual video time.
                    if let Some(start_ts) = start_ts {
                        raw_ts += start_ts;
                    }

                    // The raw timestamp from Chromaprint needs to be corrected based on current resampler delay.
                    // This tells us when the audio would _actually_ have played (in absolute time) if it were playing
                    // at the original rate.
                    //
                    // This is particularly important when the input sample rate is not divisible by the output rate,
                    // as some samples would be discarded from every frame, causing significant drift over time between
                    // the output audio and original video stream.
                    let ts = if let Some(delay) = delay {
                        raw_ts + delay
                    } else {
                        raw_ts
                    };

                    let hash = simhash32(raw_fingerprint.get());

                    hashes.push((hash, ts));
                }
            }
        }

        hashes
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
    fn sliding_window_analyzer<'a>(
        src: &'a [(u32, Duration)],
        dst: &'a [(u32, Duration)],
        threshold: Option<u32>,
        heap: &mut ComparatorHeap<'a>,
    ) {
        let threshold = threshold.unwrap_or(Self::CHROMAPRINT_MATCH_THRESHOLD);

        let mut n = 1;

        while n <= dst.len() {
            let src_end = usize::min(n, src.len());
            let dst_start = dst.len() - n;
            let src_hashes = &src[..src_end];
            let dst_hashes = &dst[dst_start..];

            let mut count = 0;

            let mut in_run = false;
            let mut run_len = 0;
            let mut max_run_len = 0;
            let mut src_run_start_idx = 0;
            let mut dst_run_start_idx = 0;
            let mut src_longest_run = &src[..];
            let mut dst_longest_run = &dst[..];

            let mut hash_data = Vec::new();

            for ((i, (src_hash, src_ts)), (j, (dst_hash, dst_ts))) in src_hashes
                .iter()
                .enumerate()
                .zip(dst_hashes.iter().enumerate())
            {
                let d = u32::count_ones(src_hash ^ dst_hash);
                if d < threshold {
                    count += 1;
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
                        src_longest_run =
                            &src_hashes[src_run_start_idx..src_run_start_idx + run_len];
                        dst_longest_run =
                            &dst_hashes[dst_run_start_idx..dst_run_start_idx + run_len];
                    }
                }
                hash_data.push((*src_ts, *dst_ts, d));
            }

            let priority = count * max_run_len;
            if priority > 0 {
                heap.push(ComparatorHeapEntry {
                    priority,
                    src_longest_run,
                    dst_longest_run,
                    hash_data,
                });
            }

            n += 1;
        }
    }

    fn find_best_match<'a>(
        src_hashes: &'a [(u32, Duration)],
        dst_hashes: &'a [(u32, Duration)],
    ) -> Option<ComparatorHeapEntry<'a>> {
        let mut heap: ComparatorHeap =
            BinaryHeap::with_capacity(src_hashes.len() + dst_hashes.len());

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
        Self::sliding_window_analyzer(src_hashes, dst_hashes, None, &mut heap);
        Self::sliding_window_analyzer(dst_hashes, src_hashes, None, &mut heap);

        heap.pop()
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        let (src_stream, dst_stream) = (self.src_stream(), self.dst_stream());
        let src_stream_idx = src_stream.index();
        let dst_stream_idx = dst_stream.index();
        let mut src_decoder = self.src_decoder()?;
        let mut dst_decoder = self.dst_decoder()?;

        // Compute hashes for both files in 3 second chunks.
        let src_frame_hashes = Self::process_frames(
            &mut self.src_ctx,
            &mut src_decoder,
            src_stream_idx,
            Some(Duration::from_secs(3)),
            None,
            None,
            Some("f1.raw"),
        );
        let dst_frame_hashes = Self::process_frames(
            &mut self.dst_ctx,
            &mut dst_decoder,
            dst_stream_idx,
            Some(Duration::from_secs(3)),
            None,
            None,
            Some("f2.raw"),
        );

        // for ((h1, t1), (h2, t2)) in src_frame_hashes.iter().zip(dst_frame_hashes.iter()) {
        //     tracing::info!(
        //         t1 = t1.as_millis() as u64,
        //         t2 = t2.as_millis() as u64,
        //         h1 = h1,
        //         h2 = h2,
        //         similarity = u32::count_ones(h1 ^ h2),
        //     );
        // }

        // We partition the hashes into opening and ending. The assumption is that the opening exists in the
        // first 75% of the video and the ending exists in the last 25%.
        let src_partition_idx = (src_frame_hashes.len() as f32 * 0.75) as usize;
        let dst_partition_idx = (dst_frame_hashes.len() as f32 * 0.75) as usize;
        let (src_opening_hashes, src_ending_hashes) = src_frame_hashes.split_at(src_partition_idx);
        let (dst_opening_hashes, dst_ending_hashes) = dst_frame_hashes.split_at(dst_partition_idx);

        let opening = Self::find_best_match(src_opening_hashes, dst_opening_hashes).unwrap();
        let ending = Self::find_best_match(src_ending_hashes, dst_ending_hashes).unwrap();
        println!("{}", opening);
        println!("{}", ending);

        Ok(())
    }
}
