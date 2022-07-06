use std::ffi::CStr;
use std::path::PathBuf;
use std::time::Duration;

use needle::audio;

#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum NeedleError {
    None = 0,
    InvalidUtf8String,
    NullArgument,
    ComparatorMinimumPaths,
}

#[no_mangle]
pub extern "C" fn needle_error_to_str(error: NeedleError) -> *const libc::c_char {
    match error {
        NeedleError::None => unsafe {
            CStr::from_bytes_with_nul_unchecked("No error\0".as_bytes()).as_ptr()
        },
        NeedleError::InvalidUtf8String => unsafe {
            CStr::from_bytes_with_nul_unchecked("Invalid UTF-8 string\0".as_bytes()).as_ptr()
        },
        NeedleError::NullArgument => unsafe {
            CStr::from_bytes_with_nul_unchecked("Input argument is null\0".as_bytes()).as_ptr()
        },
        NeedleError::ComparatorMinimumPaths => unsafe {
            CStr::from_bytes_with_nul_unchecked(
                "Comparator requires at least 2 video paths\0".as_bytes(),
            )
            .as_ptr()
        },
    }
}

unsafe fn get_paths_from_raw(
    raw_paths: *const *const libc::c_char,
    len: libc::size_t,
) -> Option<Vec<PathBuf>> {
    let raw_paths = std::slice::from_raw_parts(raw_paths, len);
    let mut paths: Vec<PathBuf> = Vec::new();
    for path in raw_paths {
        let path = std::ffi::CStr::from_ptr(*path);
        let path = match path.to_str() {
            Ok(p) => p,
            Err(_) => return None,
        };
        paths.push(path.into());
    }
    Some(paths)
}

#[derive(Debug, Default)]
pub struct Analyzer(audio::Analyzer<PathBuf>);

#[no_mangle]
pub unsafe extern "C" fn needle_audio_analyzer_new(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    threaded_decoding: bool,
    force: bool,
    output: *mut *const Analyzer,
) -> NeedleError {
    if paths.is_null() || output.is_null() {
        return NeedleError::NullArgument;
    }

    let paths = match get_paths_from_raw(paths, num_paths) {
        Some(v) => v,
        None => return NeedleError::InvalidUtf8String,
    };

    let analyzer = audio::Analyzer::from_files(paths, threaded_decoding, force);

    *output = Box::into_raw(Box::new(Analyzer(analyzer)));

    NeedleError::None
}

#[no_mangle]
pub extern "C" fn needle_audio_analyzer_free(analyzer: *const Analyzer) {
    if analyzer == std::ptr::null_mut() {
        return;
    }
    let analyzer = unsafe { Box::from_raw(analyzer as *mut Analyzer) };
    drop(analyzer);
}

#[no_mangle]
pub unsafe extern "C" fn needle_audio_analyzer_run(
    _analyzer: *mut Analyzer,
    _hash_period: f32,
    _hash_duration: f32,
    _persist: bool,
) -> NeedleError {
    todo!()
}

#[derive(Debug, Default)]
pub struct Comparator(audio::Comparator<PathBuf>);

#[no_mangle]
pub unsafe extern "C" fn needle_audio_comparator_new(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    hash_match_threshold: u16,
    opening_search_percentage: f32,
    ending_search_percentage: f32,
    min_opening_duration: f32,
    min_ending_duration: f32,
    time_padding: f32,
    output: *mut *const Comparator,
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

    let min_opening_duration = Duration::from_secs_f32(min_opening_duration);
    let min_ending_duration = Duration::from_secs_f32(min_ending_duration);
    let time_padding = Duration::from_secs_f32(time_padding);
    let comparator = audio::Comparator::from_files(
        paths,
        hash_match_threshold,
        opening_search_percentage,
        ending_search_percentage,
        min_opening_duration,
        min_ending_duration,
        time_padding,
    );

    *output = Box::into_raw(Box::new(Comparator(comparator)));

    NeedleError::None
}

#[no_mangle]
pub extern "C" fn needle_audio_comparator_free(comparator: *const Comparator) {
    if comparator == std::ptr::null_mut() {
        return;
    }
    let comparator = unsafe { Box::from_raw(comparator as *mut Comparator) };
    drop(comparator);
}

#[no_mangle]
pub unsafe extern "C" fn needle_audio_comparator_run(
    _comparator: *mut Comparator,
    _analyze: bool,
    _display: bool,
    _use_skip_files: bool,
) -> NeedleError {
    todo!()
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
        let error = unsafe {
            needle_audio_analyzer_new(path_ptrs.as_ptr(), num_paths, false, false, &mut analyzer)
        };
        assert_eq!(error, NeedleError::None);
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
        let error = unsafe {
            needle_audio_comparator_new(
                path_ptrs.as_ptr(),
                num_paths,
                10,
                0.33,
                0.2,
                10.0,
                10.0,
                0.0,
                &mut comparator,
            )
        };
        assert_eq!(error, NeedleError::None);
        assert_ne!(comparator, std::ptr::null());
        needle_audio_comparator_free(comparator);
    }
}
