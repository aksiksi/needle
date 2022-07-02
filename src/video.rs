extern crate blockhash;
extern crate ffmpeg_next;
extern crate image;

use std::path::Path;
use std::time::Duration;

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

        Ok(Self {
            decoder: decoder.video()?,
            converter: None,
        })
    }

    fn format(&self) -> ffmpeg_next::format::Pixel {
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

    fn set_converter(&mut self, format: ffmpeg_next::format::Pixel) -> anyhow::Result<()> {
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

// Wraps an ffmpeg GRAY8 video frame to implement [blockhash::Image].
struct GrayFrameView<'a> {
    width: u32,
    height: u32,
    inner: &'a [u8],
}

impl<'a> blockhash::Image for GrayFrameView<'a> {
    #[inline(always)]
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[inline(always)]
    fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let (x, y, width) = (x as usize, y as usize, self.width as usize);
        let mut data = [0xFF; 4]; // alpha defaults to 0xFF
        data[0] = self.inner[y * width + x];
        data[1] = data[0];
        data[2] = data[0];
        data
    }
}

/// Compares two videos.
pub struct VideoComparator {
    src_ctx: ffmpeg_next::format::context::Input,
    dst_ctx: ffmpeg_next::format::context::Input,
}

impl VideoComparator {
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

    // Returns the blockhash of the given frame.
    #[inline(always)]
    fn hash_frame(f: &ffmpeg_next::frame::Video) -> blockhash::Blockhash144 {
        let frame_view = GrayFrameView {
            width: f.width(),
            height: f.height(),
            inner: f.data(0),
        };
        blockhash::blockhash144(&frame_view)
    }

    // Compares two frames by computing their blockhashes and returns the
    // difference (Hamming distance).
    #[allow(unused)]
    #[inline(always)]
    fn compare_two_frames(f1: &ffmpeg_next::frame::Video, f2: &ffmpeg_next::frame::Video) -> u32 {
        let d1 = Self::hash_frame(f1);
        let d2 = Self::hash_frame(f2);
        return d1.distance(&d2);
    }

    // Returns the actual presentation timestamp for this frame (i.e., timebase agnostic).
    #[allow(unused)]
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

    // Given a video stream, applies the function `F` to each frame in the stream and
    // collects the results into a `Vec`.
    //
    // `count` can be used to limit the number of frames to process. To sample fewer frames,
    // use the `skip_by` option. For example, if `skip_by` is set to 5, one in every 5 frames
    // will be processed.
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
        let mut frame_gray = ffmpeg_next::frame::Video::new(
            ffmpeg_next::format::Pixel::GRAY8,
            decoder.width(),
            decoder.height(),
        );

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
                    decoder.convert_frame(&frame, &mut frame_gray).unwrap();
                    frame_gray.set_pts(frame.pts());
                    output.push(map_frame_fn(&frame_gray, &s));
                }
            });

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
        src_decoder.set_converter(ffmpeg_next::format::Pixel::GRAY8)?;
        dst_decoder.set_converter(ffmpeg_next::format::Pixel::GRAY8)?;

        let packets = Self::get_all_packets(&mut self.src_ctx, src_stream_idx);
        tracing::debug!(num_packets = packets.len());

        Self::seek_to_timestamp(&mut self.src_ctx, src_stream_idx, Duration::from_secs(208))?;
        Self::seek_to_timestamp(&mut self.dst_ctx, dst_stream_idx, Duration::from_secs(174))?;

        let map_frame_fn = |output_prefix: Option<&'static str>| {
            move |f: &ffmpeg_next::frame::video::Video, s: &ffmpeg_next::format::stream::Stream| {
                let time_base = f64::from(s.time_base());
                let pts = f.pts().unwrap();
                let ts = Duration::from_millis((pts as f64 * time_base * 1000.0) as u64);

                if let Some(output_prefix) = output_prefix {
                    let path = format!("frames/{}_{}.png", output_prefix, pts);
                    save_frame(f, &path).unwrap();
                    tracing::debug!(output = output_prefix, pts = pts);
                }

                (Self::hash_frame(f), ts)
            }
        };

        let src_frame_hashes = Self::process_frames(
            &mut self.src_ctx,
            &mut src_decoder,
            src_stream_idx,
            Some(1000),
            Some(5),
            map_frame_fn(Some("src_gray")),
        );
        let dst_frame_hashes = Self::process_frames(
            &mut self.dst_ctx,
            &mut dst_decoder,
            dst_stream_idx,
            Some(1000),
            Some(5),
            map_frame_fn(Some("dst_gray")),
        );
        for ((h1, t1), (h2, t2)) in src_frame_hashes.iter().zip(dst_frame_hashes.iter().skip(1)) {
            tracing::debug!(
                t1 = t1.as_millis() as u64,
                t2 = t2.as_millis() as u64,
                similarity = h1.distance(h2)
            );
        }

        Ok(())
    }
}

// Save the given frame to the path. The format of the image is determined
// using the file extension.
fn save_frame<P: AsRef<Path>>(
    frame: &ffmpeg_next::frame::video::Video,
    path: P,
) -> std::result::Result<(), std::io::Error> {
    let data = frame.data(0);
    let img_buf: image::ImageBuffer<image::Luma<u8>, &[u8]> =
        image::ImageBuffer::from_raw(frame.width(), frame.height(), data).unwrap();
    img_buf.save(path).unwrap();
    Ok(())
}

// Load a frame into a `Vec` of bytes.
    #[allow(unused)]
fn load_frame<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<u8>> {
    let img_buf = image::io::Reader::open(path)?.decode()?;
    Ok(img_buf.to_luma8().to_vec())
}
