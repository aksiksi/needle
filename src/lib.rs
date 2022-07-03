use std::path::PathBuf;
use std::time::Duration;

pub mod audio;
pub mod simhash;
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
}
