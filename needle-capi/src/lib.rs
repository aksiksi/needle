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
//!     err = needle_audio_analyzer_run(analyzer, 0.3, 3.0, true);
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
use std::ffi::CStr;
use std::path::PathBuf;
use std::time::Duration;

use needle::audio;

#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum NeedleError {
    Ok = 0,
    InvalidUtf8String,
    NullArgument,
    FrameHashDataNotFound,
    InvalidFrameHashData,
    ComparatorMinimumPaths,
    AnalyzerInvalidHashPeriod,
    AnalyzerInvalidHashDuration,
    IOError,
    Unknown,
}

impl From<needle::Error> for NeedleError {
    fn from(err: needle::Error) -> Self {
        use NeedleError::*;
        match err {
            needle::Error::FrameHashDataNotFound(_) => FrameHashDataNotFound,
            needle::Error::AnalyzerMissingPaths => Unknown,
            needle::Error::BincodeError(_) => InvalidFrameHashData,
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
            CStr::from_bytes_with_nul_unchecked("Input argument is null\0".as_bytes()).as_ptr()
        },
        NeedleError::FrameHashDataNotFound => unsafe {
            CStr::from_bytes_with_nul_unchecked("Frame hash data not found on disk\0".as_bytes())
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

fn get_paths_from_raw(
    raw_paths: *const *const libc::c_char,
    len: libc::size_t,
) -> Option<Vec<PathBuf>> {
    // SAFETY: Pointer and length are user input by design, so there is not much we can do here.
    let raw_paths = unsafe { std::slice::from_raw_parts(raw_paths, len) };

    let mut paths: Vec<PathBuf> = Vec::new();
    for path in raw_paths {
        if path.is_null() {
            return None;
        }
        // SAFETY: User should be passing in a string.
        let path = unsafe { std::ffi::CStr::from_ptr(*path) };
        let path = match path.to_str() {
            Ok(p) => p,
            Err(_) => return None,
        };
        paths.push(path.into());
    }
    Some(paths)
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
/// err = needle_audio_analyzer_run(analyzer, 0.3, 3.0, true);
/// if (err != 0) {
///     printf("Failed to run analyzer: %s\n", needle_error_to_str(err));
/// }
///
/// needle_audio_analyzer_free(analyzer);
/// ```
#[derive(Debug, Default)]
pub struct NeedleAudioAnalyzer(audio::Analyzer<PathBuf>);

/// Constructs a new [NeedleAudioAnalyzer] with sane defaults.
#[no_mangle]
pub extern "C" fn needle_audio_analyzer_new_default(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    output: *mut *const NeedleAudioAnalyzer,
) -> NeedleError {
    needle_audio_analyzer_new(paths, num_paths, false, false, output)
}

/// Constructs a new [NeedleAudioAnalyzer].
#[no_mangle]
pub extern "C" fn needle_audio_analyzer_new(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    threaded_decoding: bool,
    force: bool,
    output: *mut *const NeedleAudioAnalyzer,
) -> NeedleError {
    if paths.is_null() || output.is_null() {
        return NeedleError::NullArgument;
    }

    let paths = match get_paths_from_raw(paths, num_paths) {
        Some(v) => v,
        None => return NeedleError::InvalidUtf8String,
    };

    let analyzer = audio::Analyzer::from_files(paths, threaded_decoding, force);

    // SAFETY:
    //
    // 1) Output pointer is not null.
    // 2) We are constructing the Box ourselves and then converting it into a pointer.
    unsafe {
        *output = Box::into_raw(Box::new(NeedleAudioAnalyzer(analyzer)));
    }

    NeedleError::Ok
}

#[no_mangle]
pub extern "C" fn needle_audio_analyzer_free(analyzer: *const NeedleAudioAnalyzer) {
    if analyzer == std::ptr::null_mut() {
        return;
    }
    let analyzer = unsafe { Box::from_raw(analyzer as *mut NeedleAudioAnalyzer) };
    drop(analyzer);
}

#[no_mangle]
pub extern "C" fn needle_audio_analyzer_print_paths(analyzer: *const NeedleAudioAnalyzer) {
    if analyzer.is_null() {
        return;
    }

    let analyzer = unsafe { analyzer.as_ref().unwrap() };

    for path in analyzer.0.paths() {
        println!("{}", path.display());
    }
}

#[no_mangle]
pub extern "C" fn needle_audio_analyzer_run(
    analyzer: *const NeedleAudioAnalyzer,
    hash_period: f32,
    hash_duration: f32,
    persist: bool,
) -> NeedleError {
    if analyzer.is_null() {
        return NeedleError::NullArgument;
    }
    if hash_period <= 0.0 {
        return NeedleError::AnalyzerInvalidHashPeriod;
    }
    if hash_duration < 3.0 {
        return NeedleError::AnalyzerInvalidHashDuration;
    }

    let analyzer = unsafe { analyzer.as_ref().unwrap() };

    match analyzer.0.run(hash_period, hash_duration, persist) {
        Ok(_) => NeedleError::Ok,
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
        audio::DEFAULT_HASH_MATCH_THRESHOLD,
        audio::DEFAULT_OPENING_SEARCH_PERCENTAGE,
        audio::DEFAULT_ENDING_SEARCH_PERCENTAGE,
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
    hash_match_threshold: u16,
    opening_search_percentage: f32,
    ending_search_percentage: f32,
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
        Some(v) => v,
        None => return NeedleError::InvalidUtf8String,
    };

    let min_opening_duration = Duration::from_secs(min_opening_duration as u64);
    let min_ending_duration = Duration::from_secs(min_ending_duration as u64);
    let time_padding = Duration::from_secs_f32(time_padding);
    let comparator = audio::Comparator::from_files(paths)
        .with_hash_match_threshold(hash_match_threshold as u32)
        .with_opening_search_percentage(opening_search_percentage)
        .with_ending_search_percentage(ending_search_percentage)
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

#[no_mangle]
pub extern "C" fn needle_audio_comparator_free(comparator: *const NeedleAudioComparator) {
    if comparator == std::ptr::null_mut() {
        return;
    }
    let comparator = unsafe { Box::from_raw(comparator as *mut NeedleAudioComparator) };
    drop(comparator);
}

#[no_mangle]
pub extern "C" fn needle_audio_comparator_run(
    comparator: *const NeedleAudioComparator,
    analyze: bool,
    display: bool,
    use_skip_files: bool,
    write_skip_files: bool,
) -> NeedleError {
    if comparator.is_null() {
        return NeedleError::NullArgument;
    }

    let comparator = unsafe { comparator.as_ref().unwrap() };

    match comparator.0.run(analyze, display, use_skip_files, write_skip_files) {
        Ok(_) => NeedleError::Ok,
        Err(e) => e.into(),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_analyzer() {
        let paths = ["/tmp/abcd.mkv".to_string()].map(|s| std::ffi::CString::new(s).unwrap());
        let path_ptrs: Vec<*const libc::c_char> = paths.iter().map(|s| s.as_ptr()).collect();
        let num_paths = paths.len();
        let mut analyzer = std::ptr::null();
        let error =
            needle_audio_analyzer_new(path_ptrs.as_ptr(), num_paths, false, false, &mut analyzer);
        assert_eq!(error, NeedleError::Ok);
        assert_ne!(analyzer, std::ptr::null());
        needle_audio_analyzer_free(analyzer);
    }

    #[test]
    fn test_analyzer_default() {
        let paths = ["/tmp/abcd.mkv".to_string()].map(|s| std::ffi::CString::new(s).unwrap());
        let path_ptrs: Vec<*const libc::c_char> = paths.iter().map(|s| s.as_ptr()).collect();
        let num_paths = paths.len();
        let mut analyzer = std::ptr::null();
        let error = needle_audio_analyzer_new_default(path_ptrs.as_ptr(), num_paths, &mut analyzer);
        assert_eq!(error, NeedleError::Ok);
        assert_ne!(analyzer, std::ptr::null());
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
            10,
            0.33,
            0.2,
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
