use std::time::Duration;

// Converts a timestamp in time base units into a [std::time::Duration] that
// represents the timestamp in time units.
#[allow(unused)]
fn to_timestamp(
    ctx: &ffmpeg_next::format::context::Input,
    stream_idx: usize,
    raw_timestamp: i64,
) -> Option<Duration> {
    ctx.stream(stream_idx)
        .map(|s| f64::from(s.time_base()))
        .map(|time_base| raw_timestamp as f64 * time_base * 1000.0)
        .map(|ts| Duration::from_millis(ts as u64))
}
