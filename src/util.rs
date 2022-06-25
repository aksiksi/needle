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
