use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use ffmpeg_next::format::Pixel;

const S1_PATH: &str = "/Users/aksiksi/Movies/ep1.mkv";
const S2_PATH: &str = "/Users/aksiksi/Movies/ep2.mkv";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid timestamp for seek: requested={requested:?} duration={duration:?}")]
    InvalidSeekTimestamp {
        requested: Duration,
        duration: Duration,
    },
}

/// Wraps the `ffmpeg` video decoder.
struct VideoDecoder {
    decoder: ffmpeg_next::codec::decoder::Video,
    converter: Option<ffmpeg_next::software::scaling::context::Context>,
}

impl VideoDecoder {
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

        Ok(Self {
            decoder: decoder.video()?,
            converter: None,
        })
    }

    fn format(&self) -> Pixel {
        self.decoder.format()
    }

    fn height(&self) -> u32 {
        self.decoder.height()
    }

    fn width(&self) -> u32 {
        self.decoder.width()
    }

    fn send_packet(&mut self, packet: &ffmpeg_next::packet::Packet) -> anyhow::Result<()> {
        Ok(self.decoder.send_packet(packet)?)
    }

    fn set_converter(&mut self, format: Pixel) -> anyhow::Result<()> {
        self.converter = Some(self.decoder.converter(format)?);
        Ok(())
    }

    fn convert_frame(
        &mut self,
        frame: &ffmpeg_next::frame::Video,
        converted_frame: &mut ffmpeg_next::frame::Video,
    ) -> anyhow::Result<()> {
        if let Some(converter) = &mut self.converter {
            converter.run(frame, converted_frame)?;
        }
        Ok(())
    }

    fn receive_frame(&mut self, frame: &mut ffmpeg_next::frame::Video) -> anyhow::Result<()> {
        Ok(self.decoder.receive_frame(frame)?)
    }
}

// Wraps an ffmpeg RGB24 video frame to implement [blockhash::Image].
struct RgbFrameView<'a> {
    width: u32,
    height: u32,
    inner: &'a [u8],
}

impl<'a> blockhash::Image for RgbFrameView<'a> {
    #[inline(always)]
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[inline(always)]
    fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let (x, y, width) = (x as usize, y as usize, self.width as usize);
        let mut data = [0xFF; 4]; // alpha defaults to 0xFF
        data[0] = self.inner[y * width + x];
        data[1] = self.inner[y * width + x + 1];
        data[2] = self.inner[y * width + x + 2];
        data
    }
}

/// Compares two videos.
struct VideoComparator {
    src_ctx: ffmpeg_next::format::context::Input,
    dst_ctx: ffmpeg_next::format::context::Input,
}

impl VideoComparator {
    fn new<P, Q>(src_path: P, dst_path: Q) -> anyhow::Result<Self>
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
            .best(ffmpeg_next::media::Type::Video)
            .expect("unable to find a video stream")
    }

    fn dst_stream(&self) -> ffmpeg_next::format::stream::Stream {
        self.dst_ctx
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .expect("unable to find a video stream")
    }

    fn src_decoder(&mut self) -> anyhow::Result<VideoDecoder> {
        VideoDecoder::from_stream(self.src_stream(), false)
    }

    fn dst_decoder(&mut self) -> anyhow::Result<VideoDecoder> {
        VideoDecoder::from_stream(self.dst_stream(), false)
    }

    #[inline(always)]
    fn hash_frame(f: &ffmpeg_next::frame::Video) -> blockhash::Blockhash144 {
        let frame_view = RgbFrameView {
            width: f.width(),
            height: f.height(),
            inner: f.data(0),
        };
        blockhash::blockhash144(&frame_view)
    }

    #[inline(always)]
    fn compare_two_frames(f1: &ffmpeg_next::frame::Video, f2: &ffmpeg_next::frame::Video) -> u32 {
        let d1 = Self::hash_frame(f1);
        let d2 = Self::hash_frame(f2);
        return d1.distance(&d2);
    }

    // Returns the presentation timestamp for this frame, in milliseconds.
    fn frame_timestamp(
        ctx: &mut ffmpeg_next::format::context::Input,
        stream_idx: usize,
        frame: &ffmpeg_next::frame::Video,
    ) -> Option<Duration> {
        ctx.stream(stream_idx)
            .map(|s| f64::from(s.time_base()))
            .and_then(|time_base| frame.timestamp().map(|t| t as f64 * time_base * 1000.0))
            .map(|ts| Duration::from_millis(ts as u64))
    }

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

    fn process_frames<T, F>(
        ctx: &mut ffmpeg_next::format::context::Input,
        decoder: &mut VideoDecoder,
        stream_idx: usize,
        count: Option<usize>,
        skip_by: Option<usize>,
        map_frame_fn: F,
    ) -> Vec<T>
    where
        F: Fn(&ffmpeg_next::frame::Video, &ffmpeg_next::format::stream::Stream) -> T,
    {
        let _g = tracing::span!(tracing::Level::TRACE, "process_frames", count);

        let skip_by = skip_by.unwrap_or(1);
        let mut output: Vec<T> = Vec::new();
        let mut frame =
            ffmpeg_next::frame::Video::new(decoder.format(), decoder.width(), decoder.height());
        let mut frame_rgb =
            ffmpeg_next::frame::Video::new(Pixel::RGB24, decoder.width(), decoder.height());

        ctx.packets()
            .filter(|(s, _)| s.index() == stream_idx)
            .take(count.unwrap_or(usize::MAX))
            .enumerate()
            .map(|(i, (s, mut p))| {
                if i % skip_by != 0 {
                    p.set_flags(ffmpeg_next::codec::packet::Flags::DISCARD);
                }
                (s, p)
            })
            .for_each(|(s, p)| {
                decoder.send_packet(&p).unwrap();
                while decoder.receive_frame(&mut frame).is_ok() {
                    decoder.convert_frame(&frame, &mut frame_rgb).unwrap();
                    frame_rgb.set_pts(frame.pts());
                    output.push(map_frame_fn(&frame_rgb, &s));
                }
            });

        output
    }

    fn compare(&mut self, count: usize) -> anyhow::Result<()> {
        let (src_stream, dst_stream) = (self.src_stream(), self.dst_stream());
        let src_stream_idx = src_stream.index();
        let dst_stream_idx = dst_stream.index();
        let mut src_decoder = self.src_decoder()?;
        let mut dst_decoder = self.dst_decoder()?;
        src_decoder.set_converter(Pixel::RGB24)?;
        dst_decoder.set_converter(Pixel::RGB24)?;

        Self::seek_to_timestamp(&mut self.src_ctx, src_stream_idx, Duration::from_secs(208))?;
        Self::seek_to_timestamp(&mut self.dst_ctx, dst_stream_idx, Duration::from_secs(174))?;

        let mut src_hashes: HashMap<blockhash::Blockhash144, Duration> = HashMap::new();
        let mut dst_hashes: HashMap<blockhash::Blockhash144, Duration> = HashMap::new();
        let map_frame_fn = |f: &ffmpeg_next::frame::Video,
                            s: &ffmpeg_next::format::stream::Stream| {
            let time_base = f64::from(s.time_base());
            let ts = Duration::from_millis((f.pts().unwrap() as f64 * time_base * 1000.0) as u64);
            (Self::hash_frame(f), ts)
        };

        loop {
            // Process one slice of frames for source and destination videos.
            let src_results = Self::process_frames(
                &mut self.src_ctx,
                &mut src_decoder,
                src_stream_idx,
                Some(count),
                Some(5),
                map_frame_fn,
            );
            let src_duration = src_results[src_results.len() - 1].1;
            tracing::info!(src_duration = ?src_duration);

            let dst_results = Self::process_frames(
                &mut self.dst_ctx,
                &mut dst_decoder,
                dst_stream_idx,
                Some(count),
                Some(5),
                map_frame_fn,
            );
            let dst_duration = dst_results[dst_results.len() - 1].1;
            tracing::info!(dst_duration = ?dst_duration);

            if src_results.len() == 0 || dst_results.len() == 0 {
                break;
            }

            src_results.into_iter().for_each(|(h, d)| {
                src_hashes.insert(h, d);
            });
            dst_results.into_iter().for_each(|(h, d)| {
                dst_hashes.insert(h, d);
            });
        }

        Ok(())
    }
}

fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    ffmpeg_next::init().unwrap();
    let mut comparator = VideoComparator::new(S1_PATH, S2_PATH).unwrap();
    comparator.compare(1000).unwrap();
}
