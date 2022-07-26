#![deny(missing_docs)]

//! # needle
//!
//! needle detects openings/intros and endings/credits across video files. needle can be used standalone
//! via a dedicated CLI, or as a library to implement higher-level tools or plugins (e.g., for intro skipping).
//!
//! There are two central structs:
//!
//! 1. [Analyzer](crate::audio::Analyzer): Decodes one or more videos and converts them into a set of [FrameHashes](crate::audio::FrameHashes).
//! 2. [crate::audio::Comparator]: Searches for openings and endings across two or more videos.

use std::path::PathBuf;

/// Detects opening and endings across videos using just audio streams.
pub mod audio;
#[cfg(feature = "video")]
/// Detects opening and endings across videos using just video streams.
pub mod video;
/// Common utility functions.
pub mod util;

/// Common error type.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Frame hash data was not found on disk.
    #[error("frame hash data not found at: {0:?}")]
    FrameHashDataNotFound(PathBuf),
    /// No paths were provided to the [crate::audio::Analyzer].
    #[error("no paths provided to analyzer")]
    AnalyzerMissingPaths,
    /// Invalid path.
    #[error("path does not exist: {0:?}")]
    PathNotFound(PathBuf),
    /// Wraps [ffmpeg_next::Error].
    #[error("FFmpeg error: {0}")]
    FFmpegError(#[from] ffmpeg_next::Error),
    /// Wraps [bincode::Error].
    #[error("bincode error: {0}")]
    BincodeError(#[from] bincode::Error),
    /// Wraps [serde_json::Error].
    #[error("serde_json error: {0}")]
    SerdeJSONError(#[from] serde_json::Error),
    /// Wraps [std::io::Error].
    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}

/// Common result type.
pub type Result<T> = std::result::Result<T, Error>;
