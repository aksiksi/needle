#[cfg(feature = "ffmpeg-lib")]
mod ffmpeg_lib;
#[cfg(feature = "ffmpeg-lib")]
pub use ffmpeg_lib::AudioComparator;
