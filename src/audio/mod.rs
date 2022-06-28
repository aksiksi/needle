use std::path::Path;
use std::time::Duration;

pub mod comparator;
#[cfg(feature = "ffmpeg-lib")]
mod ffmpeg_lib;
#[cfg(feature = "ffmpeg-cli")]
mod ffmpeg_cli;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("file not found")]
    FileNotFound,
    #[cfg(feature = "ffmpeg-lib")]
    #[error("ffmpeg error")]
    FfmpegError(#[from] ffmpeg_next::Error),
    #[error("invalid stream selected")]
    InvalidStream,
}

type Result<T> = std::result::Result<T, Error>;

pub struct MediaInfo {
    pub length: Duration,
}

pub trait Backend: Sized {
    fn new(path: impl AsRef<Path>) -> Result<Self>;
    fn info(&self) -> Result<MediaInfo>;
    fn select_stream(&mut self, index: usize) -> Result<()>;
}
