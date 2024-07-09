#![deny(missing_docs)]

//! A C library that wraps [needle].
//!
//! # Example
//!
//! ```c
//! #include <stdio.h>
//! #include <needle.h>
//!
//! void main() {
//!     NeedleError err;
//!     const NeedleAudioAnalyzer *analyzer = NULL;
//!     const NeedleAudioComparator *comparator = NULL;
//!
//!     char *video_paths[] = {
//!         "/tmp/abcd.mkv",
//!         "/tmp/efgh.mp4",
//!     };
//!     const int NUM_PATHS = 2;
//!
//!     // Setup the analyzer and comparator.
//!     err = needle_audio_analyzer_new_default(paths, NUM_PATHS, &analyzer);
//!     if (err != 0) {
//!         printf("Failed to create analyzer: %s\n", needle_error_to_str(err));
//!         goto done;
//!     }
//!     err = needle_audio_comparator_new_default(paths, NUM_PATHS, &comparator);
//!     if (err != 0) {
//!         printf("Failed to create comparator: %s\n", needle_error_to_str(err));
//!         goto done;
//!     }
//!
//!     // Run the analyzer.
//!     err = needle_audio_analyzer_run(analyzer, 0.3, false, true);
//!     if (err != 0) {
//!         printf("Failed to run analyzer: %s\n", needle_error_to_str(err));
//!         goto done;
//!     }
//!
//!     done:
//!     if (analyzer != NULL) {
//!         needle_audio_analyzer_free(analyzer);
//!     }
//!     if (comparator != NULL) {
//!         needle_audio_comparator_free(comparator);
//!     }
//! }
//! ```
use std::ffi::{CStr, CString};
use std::fmt::Display;
use std::path::PathBuf;
use std::time::Duration;

use needle::audio;

/// C error enum that extends [needle::Error].
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum NeedleError {
    /// No error.
    Ok = 0,
    /// Invalid UTF-8 string.
    InvalidUtf8String,
    /// One or more pointer arguments passed into the function were NULL.
    NullArgument,
    /// One or more arguments were invalid (usually zero).
    InvalidArgument,
    /// Frame hash data was not found on disk.
    FrameHashDataNotFound,
    /// Frame hash data on disk has an invalid version.
    FrameHashDataInvalidVersion,
    /// Frame hash data on disk is not valid.
    InvalidFrameHashData,
    /// Comparator needs at least two video paths.
    ComparatorMinimumPaths,
    /// Analyzer hash period specified was invalid.
    AnalyzerInvalidHashPeriod,
    /// Analyzer hash duration specified was too short.
    AnalyzerInvalidHashDuration,
    /// Wraps a [std::io::Error].
    IOError,
    /// Unknown error.
    Unknown,
}

impl Display for NeedleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NeedleError::Ok => write!(f, "No error"),
            NeedleError::InvalidUtf8String => write!(f, "Invalid UTF-8 string"),
            NeedleError::NullArgument => write!(f, "Input argument is NULL"),
            NeedleError::InvalidArgument => {
                write!(f, "One or more input arguments were invalid (usually zero)")
            }
            NeedleError::FrameHashDataNotFound => write!(f, "Frame hash data not found on disk"),
            NeedleError::FrameHashDataInvalidVersion => {
                write!(f, "Frame hash data has an invalid version")
            }
            NeedleError::InvalidFrameHashData => {
                write!(f, "Invalid frame hash data read from disk")
            }
            NeedleError::ComparatorMinimumPaths => {
                write!(f, "Comparator requires at least 2 video paths")
            }
            NeedleError::AnalyzerInvalidHashPeriod => {
                write!(f, "Analyzer hash period must be greater than 0")
            }
            NeedleError::AnalyzerInvalidHashDuration => {
                write!(f, "Analyzer hash duration must be greater than 3 seconds")
            }
            NeedleError::IOError => write!(f, "I/O error"),
            NeedleError::Unknown => write!(
                f,
                "Unknown error occurred; please re-run with logging enabled"
            ),
        }
    }
}

impl From<needle::Error> for NeedleError {
    fn from(err: needle::Error) -> Self {
        use NeedleError::*;
        eprintln!("needle error: {}", err);
        match err {
            needle::Error::FrameHashDataNotFound(_) => FrameHashDataNotFound,
            needle::Error::FrameHashDataInvalidVersion => FrameHashDataInvalidVersion,
            needle::Error::BincodeError(_) => InvalidFrameHashData,
            needle::Error::AnalyzerMissingPaths => Unknown,
            needle::Error::IOError(_) => IOError,
            _ => Unknown,
        }
    }
}

/// Returns the string representation of the given [NeedleError].
#[no_mangle]
pub extern "C" fn needle_error_to_str(error: NeedleError) -> *const libc::c_char {
    match error {
        NeedleError::Ok => unsafe {
            CStr::from_bytes_with_nul_unchecked("No error\0".as_bytes()).as_ptr()
        },
        NeedleError::InvalidUtf8String => unsafe {
            CStr::from_bytes_with_nul_unchecked("Invalid UTF-8 string\0".as_bytes()).as_ptr()
        },
        NeedleError::NullArgument => unsafe {
            CStr::from_bytes_with_nul_unchecked("Input argument is NULL\0".as_bytes()).as_ptr()
        },
        NeedleError::InvalidArgument => unsafe {
            CStr::from_bytes_with_nul_unchecked(
                "One or more input arguments were invalid (usually zero)\0".as_bytes(),
            )
            .as_ptr()
        },
        NeedleError::FrameHashDataNotFound => unsafe {
            CStr::from_bytes_with_nul_unchecked("Frame hash data not found on disk\0".as_bytes())
                .as_ptr()
        },
        NeedleError::FrameHashDataInvalidVersion => unsafe {
            CStr::from_bytes_with_nul_unchecked(
                "Frame hash data has an invalid version.\0".as_bytes(),
            )
            .as_ptr()
        },
        NeedleError::InvalidFrameHashData => unsafe {
            CStr::from_bytes_with_nul_unchecked(
                "Invalid frame hash data read from disk\0".as_bytes(),
            )
            .as_ptr()
        },
        NeedleError::ComparatorMinimumPaths => unsafe {
            CStr::from_bytes_with_nul_unchecked(
                "Comparator requires at least 2 video paths\0".as_bytes(),
            )
            .as_ptr()
        },
        NeedleError::AnalyzerInvalidHashPeriod => unsafe {
            CStr::from_bytes_with_nul_unchecked(
                "Analyzer hash period must be greater than 0\0".as_bytes(),
            )
            .as_ptr()
        },
        NeedleError::AnalyzerInvalidHashDuration => unsafe {
            CStr::from_bytes_with_nul_unchecked(
                "Analyzer hash duration must be greater than 3 seconds\0".as_bytes(),
            )
            .as_ptr()
        },
        NeedleError::IOError => unsafe {
            CStr::from_bytes_with_nul_unchecked("I/O error\0".as_bytes()).as_ptr()
        },
        NeedleError::Unknown => unsafe {
            CStr::from_bytes_with_nul_unchecked(
                "Unknown error occurred; please re-run with logging enabled\0".as_bytes(),
            )
            .as_ptr()
        },
    }
}

/// Given a list of paths (files or directories), returns the list of valid video files.
///
/// When you are done with the returned list of videos, you must call [needle_util_video_files_free]
/// to free the memory.
///
/// For more information, refer to [needle::util::find_video_files].
#[no_mangle]
pub extern "C" fn needle_util_find_video_files(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    full: bool,
    audio: bool,
    videos: *mut *const *const libc::c_char,
    num_videos: *mut libc::size_t,
) -> NeedleError {
    if paths.is_null() || videos.is_null() || num_videos.is_null() {
        return NeedleError::NullArgument;
    }
    if num_paths == 0 {
        return NeedleError::InvalidArgument;
    }

    let paths = match get_paths_from_raw(paths, num_paths) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let video_files = match needle::util::find_video_files(&paths, full, audio) {
        Ok(v) => v,
        Err(e) => return e.into(),
    };

    let video_files = video_files
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .map(|s| CString::new(s).expect("found NULL byte in path"))
        .map(|s| s.into_raw() as *const _)
        .collect::<Vec<_>>()
        .into_boxed_slice();

    // SAFETY:
    //
    // 1) Output pointer is not null.
    // 2) We are constructing a boxed slice ourselves and then converting it into a pointer.
    unsafe {
        *num_videos = video_files.len();
        *videos = video_files.as_ptr();
    }

    // Since ownership has been transferred to the caller, we need to tell the compiler to not
    // drop this memory.
    std::mem::forget(video_files);

    NeedleError::Ok
}

/// Free the list of videos files returned by [needle_util_find_video_files].
#[no_mangle]
pub extern "C" fn needle_util_video_files_free(
    videos: *const *const libc::c_char,
    num_videos: libc::size_t,
) {
    if videos.is_null() || num_videos == 0 {
        return;
    }

    // Reconstruct a Vec from the provided pointer.
    // SAFETY: We are assuming that the user provided a _valid_ pointer here.
    let video_files = unsafe {
        // The length and capacity are the same because we used [Vec::into_boxed_slice] during creation.
        Vec::from_raw_parts(videos as *mut *mut libc::c_char, num_videos, num_videos)
    };

    // Reconstruct the original `Vec<CString>` and drop it.
    let video_files = video_files
        .into_iter()
        .map(|r| unsafe { CString::from_raw(r) })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    drop(video_files);
}

fn get_paths_from_raw(
    raw_paths: *const *const libc::c_char,
    len: libc::size_t,
) -> Result<Vec<PathBuf>, NeedleError> {
    // SAFETY: Pointer and length are user input by design, so there is not much we can do here.
    let raw_paths = unsafe { std::slice::from_raw_parts(raw_paths, len) };

    let mut paths: Vec<PathBuf> = Vec::new();
    for path in raw_paths {
        if path.is_null() {
            return Err(NeedleError::NullArgument);
        }
        // SAFETY: User should be passing in a string.
        let path = unsafe { std::ffi::CStr::from_ptr(*path) };
        let path = match path.to_str() {
            Ok(p) => p,
            Err(_) => return Err(NeedleError::InvalidUtf8String),
        };
        paths.push(path.into());
    }
    Ok(paths)
}

/// TODO
#[derive(Debug)]
pub struct FrameHashes(audio::FrameHashes);

impl From<audio::FrameHashes> for FrameHashes {
    fn from(inner: audio::FrameHashes) -> Self {
        Self(inner)
    }
}

/// Wraps [needle::audio::Analyzer] with a C API.
///
/// # Example
///
/// ```c
/// #include <stdio.h>
/// #include <needle.h>
///
/// NeedleError err;
/// const NeedleAudioAnalyzer *analyzer = NULL;
///
/// char *video_paths[] = {
///     "/tmp/abcd.mkv",
///     "/tmp/efgh.mp4",
/// };
/// const int NUM_PATHS = 2;
///
/// err = needle_audio_analyzer_new(paths, NUM_PATHS, false, false, &analyzer);
/// if (err != 0) {
///     printf("Failed to create analyzer: %s\n", needle_error_to_str(err));
///     return;
/// }
///
/// err = needle_audio_analyzer_run(analyzer, 0.3, false, true);
/// if (err != 0) {
///     printf("Failed to run analyzer: %s\n", needle_error_to_str(err));
/// }
///
/// needle_audio_analyzer_free(analyzer);
/// ```
#[derive(Debug, Default)]
pub struct NeedleAudioAnalyzer {
    inner: audio::Analyzer<PathBuf>,
    frame_hashes: Vec<FrameHashes>,
}

/// Constructs a new [NeedleAudioAnalyzer] with sane defaults.
#[no_mangle]
pub extern "C" fn needle_audio_analyzer_new_default(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    output: *mut *mut NeedleAudioAnalyzer,
) -> NeedleError {
    needle_audio_analyzer_new(
        paths,
        num_paths,
        needle::audio::DEFAULT_OPENING_SEARCH_PERCENTAGE,
        needle::audio::DEFAULT_ENDING_SEARCH_PERCENTAGE,
        false,
        false,
        false,
        output,
    )
}

/// Constructs a new [NeedleAudioAnalyzer].
#[no_mangle]
pub extern "C" fn needle_audio_analyzer_new(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    opening_search_percentage: f32,
    ending_search_percentage: f32,
    include_endings: bool,
    threaded_decoding: bool,
    force: bool,
    output: *mut *mut NeedleAudioAnalyzer,
) -> NeedleError {
    if paths.is_null() || output.is_null() {
        return NeedleError::NullArgument;
    }

    let paths = match get_paths_from_raw(paths, num_paths) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let analyzer = audio::Analyzer::from_files(paths, threaded_decoding, force)
        .with_opening_search_percentage(opening_search_percentage)
        .with_ending_search_percentage(ending_search_percentage)
        .with_include_endings(include_endings);

    // SAFETY:
    //
    // 1) Output pointer is not null.
    // 2) We are constructing the Box ourselves and then converting it into a pointer.
    unsafe {
        *output = Box::into_raw(Box::new(NeedleAudioAnalyzer {
            inner: analyzer,
            frame_hashes: vec![],
        }));
    }

    NeedleError::Ok
}

/// Returns the [FrameHashes] for the video at the given index.
///
/// Note that this index must match the index of the video provided to the [Analyzer].
#[no_mangle]
pub extern "C" fn needle_audio_analyzer_get_frame_hashes(
    analyzer: *const NeedleAudioAnalyzer,
    index: libc::size_t,
    output: *mut *const FrameHashes,
) -> NeedleError {
    if analyzer.is_null() || output.is_null() {
        return NeedleError::NullArgument;
    }

    let analyzer = unsafe { analyzer.as_ref().unwrap() };

    if index >= analyzer.frame_hashes.len() {
        return NeedleError::InvalidArgument;
    }

    unsafe {
        *output = &analyzer.frame_hashes[index] as *const _;
    }

    NeedleError::Ok
}

/// Free the provided [NeedleAudioAnalyzer].
#[no_mangle]
pub extern "C" fn needle_audio_analyzer_free(analyzer: *const NeedleAudioAnalyzer) {
    if analyzer.is_null() {
        return;
    }
    // SAFETY: We assume that the user is passing in a _valid_ pointer. Otherwise, all bets are off.
    let analyzer = unsafe { Box::from_raw(analyzer as *mut NeedleAudioAnalyzer) };
    drop(analyzer);
}

/// Print the video paths tracked by this [NeedleAudioAnalyzer].
#[no_mangle]
pub extern "C" fn needle_audio_analyzer_print_paths(analyzer: *const NeedleAudioAnalyzer) {
    if analyzer.is_null() {
        return;
    }

    // SAFETY: We assume that the user is passing in a _valid_ pointer. Otherwise, all bets are off.
    let analyzer = unsafe { analyzer.as_ref().unwrap() };

    for path in analyzer.inner.videos() {
        println!("{}", path.display());
    }
}

/// Run the [NeedleAudioAnalyzer].
#[no_mangle]
pub extern "C" fn needle_audio_analyzer_run(
    analyzer: *mut NeedleAudioAnalyzer,
    hash_duration: f32,
    persist: bool,
    threading: bool,
) -> NeedleError {
    if analyzer.is_null() {
        return NeedleError::NullArgument;
    }
    if hash_duration <= 0.0 {
        return NeedleError::AnalyzerInvalidHashDuration;
    }

    let hash_duration = Duration::from_secs_f32(hash_duration);

    // SAFETY: We assume that the user is passing in a _valid_ pointer. Otherwise, all bets are off.
    let analyzer = unsafe { analyzer.as_mut().unwrap() };

    match analyzer.inner.run(hash_duration, persist, threading) {
        Ok(frame_hashes) => {
            // Store the frame hashes for later use.
            analyzer.frame_hashes = frame_hashes.into_iter().map(|f| f.into()).collect();
            NeedleError::Ok
        }
        Err(e) => e.into(),
    }
}

/// Wraps [needle::audio::Comparator] with a C API.
///
/// # Example
///
/// ```c
/// #include <stdio.h>
/// #include <needle.h>
///
/// NeedleError err;
/// const NeedleAudioComparator *comparator = NULL;
///
/// char *video_paths[] = {
///     "/tmp/abcd.mkv",
///     "/tmp/efgh.mp4",
/// };
/// const int NUM_PATHS = 2;
///
/// err = needle_audio_comparator_new(paths, NUM_PATHS,
///                                   10,
///                                   0.33,
///                                   0.25,
///                                   20,
///                                   10,
///                                   0.0,
///                                   &comparator);
/// if (err != 0) {
///     printf("Failed to create comparator: %s\n", needle_error_to_str(err));
///     return;
/// }
///
/// err = needle_audio_comparator_run(comparator, ...);
/// if (err != 0) {
///     printf("Failed to run comparator: %s\n", needle_error_to_str(err));
/// }
///
/// needle_audio_comparator_free(comparator);
/// ```
#[derive(Debug, Default)]
pub struct NeedleAudioComparator(audio::Comparator<PathBuf>);

/// Constructs a new [NeedleAudioComparator] using sane defaults.
///
/// Refer to the library to see these values.
#[no_mangle]
pub extern "C" fn needle_audio_comparator_new_default(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    output: *mut *const NeedleAudioComparator,
) -> NeedleError {
    needle_audio_comparator_new(
        paths,
        num_paths,
        false,
        audio::DEFAULT_HASH_MATCH_THRESHOLD,
        audio::DEFAULT_MIN_OPENING_DURATION,
        audio::DEFAULT_MIN_ENDING_DURATION,
        0.0,
        output,
    )
}

/// Constructs a new [NeedleAudioComparator].
#[no_mangle]
pub extern "C" fn needle_audio_comparator_new(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    include_endings: bool,
    hash_match_threshold: u16,
    min_opening_duration: u16,
    min_ending_duration: u16,
    time_padding: f32,
    output: *mut *const NeedleAudioComparator,
) -> NeedleError {
    if paths.is_null() || output.is_null() {
        return NeedleError::NullArgument;
    }
    if num_paths < 2 {
        return NeedleError::ComparatorMinimumPaths;
    }

    let paths = match get_paths_from_raw(paths, num_paths) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let min_opening_duration = Duration::from_secs(min_opening_duration as u64);
    let min_ending_duration = Duration::from_secs(min_ending_duration as u64);
    let time_padding = Duration::from_secs_f32(time_padding);
    let comparator = audio::Comparator::from_files(paths)
        .with_include_endings(include_endings)
        .with_hash_match_threshold(hash_match_threshold as u32)
        .with_min_opening_duration(min_opening_duration)
        .with_min_ending_duration(min_ending_duration)
        .with_time_padding(time_padding);

    // SAFETY:
    //
    // 1) Output pointer is not null.
    // 2) We are constructing the Box ourselves and then converting it into a pointer.
    unsafe {
        *output = Box::into_raw(Box::new(NeedleAudioComparator(comparator)));
    }

    NeedleError::Ok
}

/// Free the provided [NeedleAudioComparator].
#[no_mangle]
pub extern "C" fn needle_audio_comparator_free(comparator: *const NeedleAudioComparator) {
    if comparator.is_null() {
        return;
    }
    // SAFETY: We assume that the user is passing in a _valid_ pointer. Otherwise, all bets are off.
    let comparator = unsafe { Box::from_raw(comparator as *mut NeedleAudioComparator) };
    drop(comparator);
}

/// Run the [NeedleAudioComparator].
#[no_mangle]
pub extern "C" fn needle_audio_comparator_run(
    comparator: *const NeedleAudioComparator,
    analyze: bool,
    display: bool,
    use_skip_files: bool,
    write_skip_files: bool,
    threading: bool,
) -> NeedleError {
    if comparator.is_null() {
        return NeedleError::NullArgument;
    }

    // SAFETY: We assume that the user is passing in a _valid_ pointer. Otherwise, all bets are off.
    let comparator = unsafe { comparator.as_ref().unwrap() };

    match comparator.0.run(
        analyze,
        display,
        use_skip_files,
        write_skip_files,
        threading,
        // TODO(aksiksi): Make this an argument.
        false,
    ) {
        Ok(_) => NeedleError::Ok,
        Err(e) => e.into(),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_sample_paths() -> Vec<PathBuf> {
        let resources = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("needle")
            .join("resources");
        vec![
            resources.join("sample-5s.mp4"),
            resources.join("sample-shifted-4s.mp4"),
        ]
    }

    #[test]
    fn test_find_video_files() {
        let paths: Vec<CString> = get_sample_paths()
            .into_iter()
            .map(|p| CString::new(p.to_string_lossy().to_string()).unwrap())
            .collect();
        let path_ptrs: Vec<*const libc::c_char> = paths
            .iter()
            .map(|s| s.as_ptr() as *const libc::c_char)
            .collect();

        let mut videos = std::ptr::null();
        let mut num_videos = 0usize;

        let error = needle_util_find_video_files(
            path_ptrs.as_ptr(),
            paths.len(),
            true,
            true,
            &mut videos,
            &mut num_videos,
        );
        assert_eq!(error, NeedleError::Ok);
        needle_util_video_files_free(videos, num_videos);
    }

    #[test]
    fn test_analyzer() {
        let paths = ["/tmp/abcd.mkv".to_string()].map(|s| std::ffi::CString::new(s).unwrap());
        let path_ptrs: Vec<*const libc::c_char> = paths.iter().map(|s| s.as_ptr()).collect();
        let num_paths = paths.len();
        let mut analyzer = std::ptr::null_mut();
        let error = needle_audio_analyzer_new_default(path_ptrs.as_ptr(), num_paths, &mut analyzer);
        assert_eq!(error, NeedleError::Ok);
        assert_ne!(analyzer, std::ptr::null_mut());
        needle_audio_analyzer_free(analyzer);
    }

    #[test]
    fn test_analyzer_default() {
        let paths = ["/tmp/abcd.mkv".to_string()].map(|s| std::ffi::CString::new(s).unwrap());
        let path_ptrs: Vec<*const libc::c_char> = paths.iter().map(|s| s.as_ptr()).collect();
        let num_paths = paths.len();
        let mut analyzer = std::ptr::null_mut();
        let error = needle_audio_analyzer_new_default(path_ptrs.as_ptr(), num_paths, &mut analyzer);
        assert_eq!(error, NeedleError::Ok);
        assert_ne!(analyzer, std::ptr::null_mut());
        needle_audio_analyzer_free(analyzer);
    }

    #[test]
    fn test_comparator() {
        let paths = ["/tmp/abcd.mkv".to_string(), "/tmp/efgh.mp4".to_string()]
            .map(|s| std::ffi::CString::new(s).unwrap());
        let path_ptrs: Vec<*const libc::c_char> = paths.iter().map(|s| s.as_ptr()).collect();
        let num_paths = paths.len();
        let mut comparator = std::ptr::null();
        let error = needle_audio_comparator_new(
            path_ptrs.as_ptr(),
            num_paths,
            false,
            10,
            10,
            10,
            0.0,
            &mut comparator,
        );
        assert_eq!(error, NeedleError::Ok);
        assert_ne!(comparator, std::ptr::null());
        needle_audio_comparator_free(comparator);
    }

    #[test]
    fn test_comparator_default() {
        let paths = ["/tmp/abcd.mkv".to_string(), "/tmp/efgh.mp4".to_string()]
            .map(|s| std::ffi::CString::new(s).unwrap());
        let path_ptrs: Vec<*const libc::c_char> = paths.iter().map(|s| s.as_ptr()).collect();
        let num_paths = paths.len();
        let mut comparator = std::ptr::null();
        let error =
            needle_audio_comparator_new_default(path_ptrs.as_ptr(), num_paths, &mut comparator);
        assert_eq!(error, NeedleError::Ok);
        assert_ne!(comparator, std::ptr::null());
        needle_audio_comparator_free(comparator);
    }
}
