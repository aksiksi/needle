use std::path::Path;

use ffmpeg_next::format::Pixel;
use rgb::FromSlice;

const S1_PATH: &str = "/Users/aksiksi/Movies/ep1.mkv";
const S2_PATH: &str = "/Users/aksiksi/Movies/ep2.mkv";

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

    fn receive_frame_and_convert(
        &mut self,
        frame: &mut ffmpeg_next::frame::Video,
        converted_frame: &mut ffmpeg_next::frame::Video,
    ) -> anyhow::Result<()> {
        self.receive_frame(frame)?;
        self.convert_frame(frame, converted_frame)?;
        Ok(())
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
    fn hash_frame(f: &ffmpeg_next::frame::Video) -> md5::Digest {
        md5::compute(f.data(0))
    }

    fn compare_two_frames(f1: &ffmpeg_next::frame::Video, f2: &ffmpeg_next::frame::Video) -> bool {
        let d1 = Self::hash_frame(f1);
        let d2 = Self::hash_frame(f2);
        return d1 == d2;
    }

    fn seek_to_timestamp(
        ctx: &mut ffmpeg_next::format::context::Input,
        stream_idx: usize,
        timestamp: f64, // in seconds
    ) -> anyhow::Result<()> {
        let time_base: f64 = ctx.stream(stream_idx).unwrap().time_base().into();
        let timestamp = (timestamp / time_base) as i64;
        ctx.seek_to_frame(stream_idx as i32, timestamp, ffmpeg_next::format::context::input::SeekFlags::empty())?;
        Ok(())
    }

    fn find_next_key_frame(
        ctx: &mut ffmpeg_next::format::context::Input,
        decoder: &mut VideoDecoder,
        stream_idx: usize,
        frame_buf: &mut ffmpeg_next::frame::Video,
        num_frames_to_skip: usize,
    ) -> anyhow::Result<i64> {
        let mut frame_count = 0;

        for (s, p) in ctx.packets() {
            if s.index() != stream_idx {
                continue;
            }
            // if !p.is_key() {
            //     continue;
            // }
            decoder.send_packet(&p)?;
            while decoder.receive_frame(frame_buf).is_ok() {
                frame_count += 1;
                if frame_count == num_frames_to_skip - 1 {
                    return Ok(dbg!(frame_buf.timestamp().unwrap_or(0)));
                }
            }
        }

        Ok(0)
    }

    fn compare(&mut self, count: usize) -> anyhow::Result<()> {
        let src_stream_idx = self.src_stream().index();
        let dst_stream_idx = self.dst_stream().index();
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

        Self::seek_to_timestamp(&mut self.src_ctx, src_stream_idx, 208.0)?;
        Self::seek_to_timestamp(&mut self.dst_ctx, dst_stream_idx, 174.0)?;

        for _ in 0..count {
            let t1 = Self::find_next_key_frame(
                &mut self.src_ctx,
                &mut src_decoder,
                src_stream_idx,
                &mut src_frame,
                100,
            )?;
            let t2 = Self::find_next_key_frame(
                &mut self.dst_ctx,
                &mut dst_decoder,
                dst_stream_idx,
                &mut dst_frame,
                100,
            )?;

            src_decoder.convert_frame(&src_frame, &mut src_frame_rgb)?;
            dst_decoder.convert_frame(&dst_frame, &mut dst_frame_rgb)?;

            let d = dssim_core::Dssim::new();
            let img1 = d
                .create_image_rgb(
                    src_frame_rgb.data(0).as_rgb(),
                    src_frame_rgb.width() as usize,
                    src_frame_rgb.height() as usize,
                )
                .unwrap();
            let img2 = d
                .create_image_rgb(
                    dst_frame_rgb.data(0).as_rgb(),
                    dst_frame_rgb.width() as usize,
                    dst_frame_rgb.height() as usize,
                )
                .unwrap();
            let (val, _) = d.compare(&img1, &img2);
            dbg!(val);
        }

        Ok(())
    }
}

fn main() {
    ffmpeg_next::init().unwrap();
    let mut comparator = VideoComparator::new(S1_PATH, S2_PATH).unwrap();
    comparator.compare(100).unwrap();
}
