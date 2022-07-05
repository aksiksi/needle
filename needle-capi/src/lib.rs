use std::time::Duration;
use std::{ffi::CStr, path::PathBuf};

use needle::audio;

#[repr(C)]
pub enum Error {
    None = 0,
    InvalidUtf8String,
}

// TODO(aksiksi): Make this a macro
const fn _needle_error_to_str(error: Error) -> *const libc::c_char {
    match error {
        Error::None => unsafe {
            CStr::from_bytes_with_nul_unchecked("No error\0".as_bytes()).as_ptr()
        },
        Error::InvalidUtf8String => unsafe {
            CStr::from_bytes_with_nul_unchecked("Invalid UTF-8 string\0".as_bytes()).as_ptr()
        },
    }
}

#[no_mangle]
pub extern "C" fn needle_error_to_str(error: Error) -> *const libc::c_char {
    _needle_error_to_str(error)
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

pub struct Analyzer(audio::Analyzer<PathBuf>);

#[no_mangle]
pub unsafe extern "C" fn needle_audio_analyzer_new(
    paths: *const *const libc::c_char,
    num_paths: libc::size_t,
    threaded_decoding: bool,
    force: bool,
    output: *mut *const Analyzer,
) -> Error {
    let paths = match get_paths_from_raw(paths, num_paths) {
        Some(v) => v,
        None => return Error::InvalidUtf8String,
    };

    let analyzer = audio::Analyzer::from_files(paths, threaded_decoding, force);

    *output = Box::into_raw(Box::new(Analyzer(analyzer)));

    Error::None
}

#[no_mangle]
pub unsafe extern "C" fn needle_audio_analyzer_run(
    analyzer: *mut Analyzer,
    hash_period: f32,
    hash_duration: f32,
    persist: bool,
) -> Error {
    todo!()
}

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
) -> Error {
    let paths = match get_paths_from_raw(paths, num_paths) {
        Some(v) => v,
        None => return Error::InvalidUtf8String,
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

    Error::None
}

#[no_mangle]
pub unsafe extern "C" fn needle_audio_comparator_run(
    comparator: *mut Comparator,
    analyze: bool,
    display: bool,
    use_skip_files: bool,
) -> Error {
    todo!()
}
