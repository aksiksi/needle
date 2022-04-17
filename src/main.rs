use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use ffmpeg_next::format::Pixel;

const S1_PATH: &str = "/Users/aksiksi/Movies/ep1.mkv";
const S2_PATH: &str = "/Users/aksiksi/Movies/ep2.mkv";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid timestamp for seek: requested={requested:?} duration={duration:?}")]
    InvalidSeekTimestamp { requested: Duration, duration: Duration },
}

/// Wraps the `ffmpeg` video decoder.
struct VideoDecoder {
    decoder: ffmpeg_next::codec::decoder::Video,
    converter: Option<ffmpeg_next::software::scaling::context::Context>,
}

impl VideoDecoder {
    fn from_stream(stream: ffmpeg_next::format::stream::Stream) -> anyhow::Result<Self> {
        let decoder = ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())?
            .decoder()
            .video()?;
        Ok(Self {
            decoder,
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

// Wraps an RGB video frame to implement [blockhash::Image].
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
        VideoDecoder::from_stream(self.src_stream())
    }

    fn dst_decoder(&mut self) -> anyhow::Result<VideoDecoder> {
        VideoDecoder::from_stream(self.dst_stream())
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

    fn find_next_frame(
        ctx: &mut ffmpeg_next::format::context::Input,
        decoder: &mut VideoDecoder,
        stream_idx: usize,
        frame_buf: &mut ffmpeg_next::frame::Video,
        num_frames_to_skip: usize,
    ) -> anyhow::Result<Duration> {
        let packet_iter = ctx
            .packets()
            .filter(|(s, _)| s.index() == stream_idx)
            .map(|(_, p)| p);

        // TODO(aksiksi): Figure out why we get duplicate frames on successive calls
        // to this method.
        for (i, mut p) in packet_iter.enumerate() {
            if i < num_frames_to_skip {
                // Mark this packet for discard.
                p.set_flags(ffmpeg_next::codec::packet::Flags::DISCARD);
            }
            decoder.send_packet(&p)?;
            if decoder.receive_frame(frame_buf).is_ok() {
                break;
            }
        }

        Ok(Self::frame_timestamp(ctx, stream_idx, &frame_buf).unwrap())
    }

    fn compare(&mut self, count: usize) -> anyhow::Result<()> {
        let (src_stream, dst_stream) = (self.src_stream(), self.dst_stream());
        let src_stream_idx = src_stream.index();
        let dst_stream_idx = dst_stream.index();
        let mut src_decoder = self.src_decoder()?;
        let mut dst_decoder = self.dst_decoder()?;
        src_decoder.set_converter(Pixel::RGB24)?;
        dst_decoder.set_converter(Pixel::RGB24)?;
        let mut src_frame = ffmpeg_next::frame::Video::new(
            src_decoder.format(),
            src_decoder.width(),
            src_decoder.height(),
        );
        let mut src_frame_rgb =
            ffmpeg_next::frame::Video::new(Pixel::RGB24, src_decoder.width(), src_decoder.height());
        let mut dst_frame = ffmpeg_next::frame::Video::new(
            dst_decoder.format(),
            dst_decoder.width(),
            dst_decoder.height(),
        );
        let mut dst_frame_rgb =
            ffmpeg_next::frame::Video::new(Pixel::RGB24, dst_decoder.width(), dst_decoder.height());
        let mut src_frame_hash_map = HashMap::new();

        Self::seek_to_timestamp(&mut self.src_ctx, src_stream_idx, Duration::from_secs(208))?;
        Self::seek_to_timestamp(&mut self.dst_ctx, dst_stream_idx, Duration::from_secs(174))?;

        for _ in 0..count {
            let t1 = Self::find_next_frame(
                &mut self.src_ctx,
                &mut src_decoder,
                src_stream_idx,
                &mut src_frame,
                100,
            )?;
            let t2 = Self::find_next_frame(
                &mut self.dst_ctx,
                &mut dst_decoder,
                dst_stream_idx,
                &mut dst_frame,
                100,
            )?;
            dbg!(t1, t2);

            src_decoder.convert_frame(&src_frame, &mut src_frame_rgb)?;
            dst_decoder.convert_frame(&dst_frame, &mut dst_frame_rgb)?;

            src_frame_hash_map.insert(Self::hash_frame(&src_frame_rgb), t1);

            dbg!(Self::compare_two_frames(&src_frame_rgb, &dst_frame_rgb));
        }

        Ok(())
    }
}

fn main() {
    ffmpeg_next::init().unwrap();
    let mut comparator = VideoComparator::new(S1_PATH, S2_PATH).unwrap();
    comparator.compare(100).unwrap();
}
