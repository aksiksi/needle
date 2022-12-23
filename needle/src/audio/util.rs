use std::time::Duration;

use crate::Result;

// Converts a timestamp in time base units into a [std::time::Duration] that
// represents the timestamp in time units.
pub(crate) fn to_timestamp(
    time_base: ffmpeg_next::util::rational::Rational,
    raw_timestamp: i64,
) -> Duration {
    let time_base: f64 = time_base.into();
    let ts = raw_timestamp as f64 * time_base;
    Duration::from_secs_f64(ts)
}

// Seeks the video stream to the given timestamp. Under the hood, this uses
// the standard FFmpeg function, `avformat_seek_file`.
pub(crate) fn seek_to_timestamp(
    ctx: &mut ffmpeg_next::format::context::Input,
    time_base: ffmpeg_next::util::rational::Rational,
    timestamp: Duration,
) -> Result<()> {
    let min_timestamp = timestamp - Duration::from_millis(1000);
    let max_timestamp = timestamp + Duration::from_millis(1000);

    let time_base: f64 = time_base.into();
    let duration = Duration::from_millis((ctx.duration() as f64 * time_base) as u64);
    // TODO(aksiksi): Make this an error.
    assert!(
        max_timestamp < duration,
        "timestamp must be less than the stream duration"
    );

    // Convert timestamps from ms to seconds, then divide by time_base to get each timestamp
    // in time_base units.
    let timestamp = (timestamp.as_millis() as f64 / time_base) as i64;
    let min_timestamp = (min_timestamp.as_millis() as f64 / time_base) as i64;
    let max_timestamp = (max_timestamp.as_millis() as f64 / time_base) as i64;

    Ok(ctx.seek(timestamp, min_timestamp..max_timestamp)?)
}

pub(crate) fn find_best_audio_stream(
    input: &ffmpeg_next::format::context::Input,
) -> ffmpeg_next::format::stream::Stream {
    input
        .streams()
        .best(ffmpeg_next::media::Type::Audio)
        .expect("unable to find an audio stream")
}
