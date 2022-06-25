use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "audio")]
mod simhash;
#[cfg(feature = "video")]
mod video;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid timestamp for seek: requested={requested:?} duration={duration:?}")]
    InvalidSeekTimestamp {
        requested: Duration,
        duration: Duration,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Mode {
    #[cfg(feature = "audio")]
    Audio,
    #[cfg(feature = "video")]
    Video,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_enum, default_value_t = Mode::Audio)]
    mode: Mode,

    #[clap(name = "FILE", required = true, number_of_values = 2)]
    files: Vec<PathBuf>,
}

fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    #[cfg(feature = "ffmpeg-next")]
    ffmpeg_next::init().unwrap();

    let args = Args::parse();

    match args.mode {
        #[cfg(feature = "audio")]
        Mode::Audio => {
            let mut audio_comparator = audio::AudioComparator::new(&args.files[0], &args.files[1]).unwrap();
            audio_comparator.run().unwrap();
        }
        #[cfg(feature = "video")]
        Mode::Video => {
            let mut video_comparator = video::VideoComparator::new(&args.files[0], &args.files[1]).unwrap();
            video_comparator.compare(1000).unwrap();
        }
    }
}
