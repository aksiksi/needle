use std::path::PathBuf;
use std::time::Duration;

pub mod audio;
pub mod util;
#[cfg(feature = "video")]
pub mod video;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid timestamp for seek: requested={requested:?} duration={duration:?}")]
    InvalidSeekTimestamp {
        requested: Duration,
        duration: Duration,
    },
    #[error("frame hash data not found at: {0:?}")]
    FrameHashDataNotFound(PathBuf),
    #[error("no paths provided to analyzer")]
    AnalyzerMissingPaths,
    #[error("FFmpeg error: {0}")]
    FFmpegError(#[from] ffmpeg_next::Error),
    #[error("bincode error: {0}")]
    BincodeError(#[from] bincode::Error),
    #[error("serde_json error: {0}")]
    SerdeJSONError(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
