#![deny(missing_docs)]

//! # needle
//!
//! needle detects openings/intros and endings/credits across video files. needle can be used standalone
//! via a dedicated CLI, or as a library to implement higher-level tools or plugins (e.g., for intro skipping).
//!
//! The library exposes two central structs:
//!
//! 1. [Analyzer](crate::audio::Analyzer): Decodes one or more videos and converts them into a set of [FrameHashes](crate::audio::FrameHashes).
//! 2. [Comparator](crate::audio::Comparator): Searches for openings and endings across two or more videos.
//!
//! ## Basic Usage
//!
//! First, you need to create and run an [Analyzer](crate::audio::Analyzer).
//!
//! This will decode the audio streams for all provided video files and return a list of [FrameHashes](audio::FrameHashes), one per video. The structure
//! stores a compressed representation of the audio stream that contains _only_ the data we need to search for openings and endings.
//!
//! ```
//! use std::path::PathBuf;
//! use needle::audio::Analyzer;
//! # fn get_sample_paths() -> Vec<PathBuf> {
//! #     let resources = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
//! #     vec![
//! #         resources.join("sample-5s.mp4"),
//! #         resources.join("sample-shifted-4s.mp4"),
//! #     ]
//! # }
//!
//! let video_paths: Vec<PathBuf> = get_sample_paths();
//! let analyzer = Analyzer::from_files(video_paths, false, false);
//!
//! // Use a `hash_period` of 1.0, `hash_duration` of 3.0, do not `persist` frame hash data
//! // and enable `threading`.
//! let frame_hashes = analyzer.run(1.0, 3.0, false, true).unwrap();
//! ```
//!
//! Now you need to create and run a [Comparator](crate::audio::Comparator) using the output [FrameHashes](audio::FrameHashes). You can re-use
//! the videos by constructing an instance from the [Analyzer](crate::audio::Analyzer).
//!
//! ```
//! use std::path::PathBuf;
//! use needle::audio::Comparator;
//! # use needle::audio::Analyzer;
//! # fn get_sample_paths() -> Vec<PathBuf> {
//! #     let resources = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
//! #     vec![
//! #         resources.join("sample-5s.mp4"),
//! #         resources.join("sample-shifted-4s.mp4"),
//! #     ]
//! # }
//! # let video_paths: Vec<PathBuf> = get_sample_paths();
//! # let analyzer = Analyzer::from_files(video_paths, false, false);
//! # let frame_hashes = analyzer.run(1.0, 3.0, false, true).unwrap();
//!
//! let comparator: Comparator = analyzer.into();
//! let results = comparator.run_with_frame_hashes(frame_hashes, true, false, false, true).unwrap();
//!
//! dbg!(results);
//! // [
//! //     SearchResult {
//! //         opening: None,
//! //         ending: Some(
//! //             (
//! //                 1331.664387072s,
//! //                 1419.024930474s,
//! //             ),
//! //         ),
//! //     },
//! //     SearchResult {
//! //         opening: Some(
//! //             (
//! //                 44.718820458s,
//! //                 131.995463634s,
//! //             ),
//! //         ),
//! //         ending: Some(
//! //             (
//! //                 1331.664387072s,
//! //                 1436.560077708s,
//! //             ),
//! //         ),
//! //     },
//! //     SearchResult {
//! //         opening: Some(
//! //             (
//! //                 41.11111074s,
//! //                 127.800452334s,
//! //             ),
//! //         ),
//! //         ending: Some(
//! //             (
//! //                 1331.664387072s,
//! //                 1436.560077708s,
//! //             ),
//! //         ),
//! //     },
//! // ]
//! ```
//!
//! [Comparator::run_with_frame_hashes](crate::audio::Comparator::run_with_frame_hashes) runs a search for openings and endings
//! using the provided frame hash data. Note that there is an equivalent method that can read existing frame hash data files from disk
//! ([Comparator::run](crate::audio::Comparator::run)).
//!
//! The output of this method is a map from each video file to a [SearchResult](crate::audio::SearchResult). This
//! structure contains the actual times for any detected openings and endings.

use std::path::PathBuf;

/// Detects opening and endings across videos using just audio streams.
pub mod audio;
/// Common utility functions.
pub mod util;
#[cfg(feature = "video")]
/// Detects opening and endings across videos using just video streams.
pub mod video;

/// Common error type.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Frame hash data was not found on disk.
    #[error("frame hash data not found at: {0:?}")]
    FrameHashDataNotFound(PathBuf),
    /// Invalid frame hash data version.
    #[error("invalid frame hash data version")]
    FrameHashDataInvalidVersion,
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
