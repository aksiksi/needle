use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{Error, Result};

/// Formats the given [Duration] as "MM:SSs"
pub fn format_time(t: Duration) -> String {
    let minutes = t.as_secs() / 60;
    let seconds = t.as_secs() % 60;
    format!("{:02}:{:02}s", minutes, seconds)
}

/// Checks if the given path points to a valid video file.
///
/// If `full` is set to **false**, only the file header will be checked. This is a very cheap
/// operation, but it does not guarantee validity. If set to **true**, FFmpeg will be used to
/// check the video contents - note that this is more expensive, but much more accurate.
///
/// If `audio` is set to true, this function will ensure that the video contains *at least* one audio stream.
/// This flag is only used when `full` is set to **true**.
pub fn is_valid_video_file(path: impl AsRef<Path>, full: bool, audio: bool) -> bool {
    if !full {
        let mut buf = [0u8; 8192];
        let mut f = std::fs::File::open(path.as_ref()).unwrap();
        f.read(&mut buf).unwrap();
        return infer::is_video(&buf);
    }

    if let Ok(input) = ffmpeg_next::format::input(&path.as_ref()) {
        let num_video_streams = input
            .streams()
            .filter(|s| s.parameters().medium() == ffmpeg_next::util::media::Type::Video)
            .count();
        let num_audio_streams = input
            .streams()
            .filter(|s| s.parameters().medium() == ffmpeg_next::util::media::Type::Audio)
            .count();
        num_video_streams > 0 && (!audio || num_audio_streams > 0)
    } else {
        false
    }
}

/// Given a list of paths (files or directories), returns the list of valid video files.
///
/// The `full` and `audio` flags are forwarded as-is to [is_valid_video_file].
///
/// Note: This function only looks for videos one directory level deep.
pub fn find_video_files<P: AsRef<Path>>(
    paths: &[P],
    full: bool,
    audio: bool,
) -> Result<Vec<PathBuf>> {
    // Validate all paths.
    for path in paths {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::PathNotFound(path.to_owned()));
        }
    }

    // Find valid video files.
    let mut valid_video_files = Vec::new();
    for path in paths {
        let path = path.as_ref();
        if path.is_dir() {
            valid_video_files.extend(
                std::fs::read_dir(path)
                    .unwrap()
                    .map(|p| {
                        let entry = p.unwrap();
                        entry.path()
                    })
                    .filter(|p| is_valid_video_file(p, full, audio))
                    .collect::<Vec<_>>(),
            );
        } else {
            if is_valid_video_file(path, full, audio) {
                valid_video_files.push(path.to_owned());
            }
        }
    }

    Ok(valid_video_files)
}

/// Computes the MD5 checksum of the file header (first 8K bytes).
pub(crate) fn compute_header_md5sum(video: impl AsRef<Path>) -> crate::Result<String> {
    let mut buf = [0u8; 8*1024];
    let mut f = std::fs::File::open(video.as_ref())?;
    f.read_exact(&mut buf)?;
    let hash = format!("{:x}", md5::compute(&buf));
    Ok(hash)
}

/// Computes the MD5 checksum of the entire video file.
///
/// This function allocates a 10M buffer on the heap and reads the file in chunks.
#[allow(unused)]
pub(crate) fn compute_md5sum(video: impl AsRef<Path>) -> crate::Result<String> {
    let mut buf = Box::new([0u8; 10*1024*1024]);
    let mut f = std::fs::File::open(video.as_ref())?;
    let mut ctx = md5::Context::new();
    loop {
        let n = f.read(buf.as_mut_slice())?;
        if n == 0 {
            break
        }
        ctx.consume(&buf[..n]);
    }
    let hash = format!("{:x}", ctx.compute());
    Ok(hash)
}

/// Returns the underlying FFmpeg version integer used by needle.
pub fn ffmpeg_version() -> u32 {
    ffmpeg_next::util::version()
}

/// Returns the underlying FFmpeg version string used by needle.
pub fn ffmpeg_version_string() -> String {
    let version_int = ffmpeg_version();

    // Reference: https://github.com/FFmpeg/FFmpeg/blob/130d19bf2044ac76372d1b97ab87ab283c8b37f8/libavutil/version.h#L64
    format!(
        "{}.{}.{}",
        version_int >> 16,             // MAJOR
        (version_int & 0x00FF00) >> 8, // MINOR
        version_int & 0xFF             // MICRO
    )
}
