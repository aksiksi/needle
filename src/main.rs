#![allow(unused)]
use std::path::Path;
use std::time::Duration;

use blockhash::Blockhash144;
use ffmpeg_next::format::Pixel;

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "video")]
mod video;

const S1_PATH: &str = "/Users/aksiksi/Movies/ep1.mkv";
const S2_PATH: &str = "/Users/aksiksi/Movies/ep2.mkv";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid timestamp for seek: requested={requested:?} duration={duration:?}")]
    InvalidSeekTimestamp {
        requested: Duration,
        duration: Duration,
    },
}

fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    #[cfg(feature = "ffmpeg-next")]
    ffmpeg_next::init().unwrap();

    // let mut video_comparator = video::VideoComparator::new(S1_PATH, S2_PATH).unwrap();
    // video_comparator.compare(1000).unwrap();

    let mut audio_comparator = audio::AudioComparator::new(S1_PATH, S2_PATH).unwrap();
    audio_comparator.compare(1000).unwrap();
}
