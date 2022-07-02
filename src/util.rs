use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[allow(unused)]
pub fn write_samples_in_range(
    path: impl AsRef<Path>,
    (start, end): (Duration, Duration),
    samples: &[(Duration, Vec<u8>)],
) {
    let mut f = std::fs::File::create(&path).unwrap();
    for (d, samples) in samples {
        if *d >= start && *d <= end {
            f.write(samples).unwrap();
        }
    }
}

pub fn format_time(t: Duration) -> String {
    let minutes = t.as_secs() / 60;
    let seconds = t.as_secs() % 60;
    format!("{:02}:{:02}s", minutes, seconds)
}

pub fn is_valid_video_file(path: impl AsRef<Path>, audio: bool) -> bool {
    if let Ok(input) = ffmpeg_next::format::input(&path.as_ref()) {
        let num_video_streams = input
            .streams()
            .filter(|s| s.parameters().medium() == ffmpeg_next::util::media::Type::Video)
            .count();
        let num_audio_streams = input
            .streams()
            .filter(|s| s.parameters().medium() == ffmpeg_next::util::media::Type::Audio)
            .count();
        num_video_streams > 0 && (!audio || num_audio_streams > 0)
    } else {
        false
    }
}

#[allow(unused)]
pub fn find_all_video_files(paths: &[impl AsRef<Path>], audio: bool) -> Vec<PathBuf> {
    let mut video_files: Vec<PathBuf> = Vec::new();
    for p in paths {
        if is_valid_video_file(p, audio) {
            video_files.push(p.as_ref().to_owned());
        }
    }
    video_files
}
