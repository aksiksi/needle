use std::io::Write;
use std::path::Path;
use std::time::Duration;

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
        // If audio is required, make sure we have at least 1 audio stream available.
        !audio || input.streams().filter(|s| {
            s.parameters().medium() == ffmpeg_next::util::media::Type::Audio
        }).count() != 0
    } else {
        false
    }
}

pub fn find_all_video_files<'a>(paths: &'a [impl AsRef<Path> + 'a], audio: bool) -> Vec<&'a Path> {
    let mut video_files: Vec<&'a Path> = Vec::new();
    for p in paths {
        if is_valid_video_file(p, audio) {
            video_files.push(p.as_ref());
        }
    }
    video_files
}
