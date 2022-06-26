use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgAction, CommandFactory, ErrorKind, Parser};

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "audio")]
mod simhash;
mod util;
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

    #[clap(short, long, default_value = "false")]
    write_result: bool,

    #[clap(
        long,
        default_value = "75",
        value_parser = clap::value_parser!(u8),
        help = "Specifies which portion of the video the opening and ending should be in. For example, if set to 75%, a match found in the first 75% of the video will be considered the opening, while a match in the last 25% will be considered the ending."
    )]
    opening_search_percentage: u8,

    #[clap(
        long,
        default_value = "10",
        value_parser = clap::value_parser!(u16),
        help = "Minimum opening duration, in seconds. Setting a value that is close to the actual length helps reduce false positives (i.e., detecting an opening when there isn't one)."
    )]
    min_opening_duration: u16,

    #[clap(
        long,
        default_value = "10",
        value_parser = clap::value_parser!(u16),
        help = "Minimum ending duration, in seconds. Setting a value that is close to the actual length helps reduce false positives (i.e., detecting an ending when there isn't one)."
    )]
    min_ending_duration: u16,

    #[clap(
        long,
        default_value = "false",
        action(ArgAction::SetTrue),
        help = "Enable multi-threaded audio decoding in ffmpeg. This will create NUM_CPU threads."
    )]
    threaded: bool,
}

fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    #[cfg(feature = "ffmpeg-next")]
    ffmpeg_next::init().unwrap();

    let args = Args::parse();

    if args.opening_search_percentage >= 100 {
        let mut cmd = Args::command();
        cmd.error(
            ErrorKind::InvalidValue,
            "opening_search_percentage must be less than 100",
        )
        .exit();
    }
    let opening_search_percentage = args.opening_search_percentage as f32 / 100.0;

    match args.mode {
        #[cfg(feature = "audio")]
        Mode::Audio => {
            let mut audio_comparator =
                audio::AudioComparator::new(&args.files[0], &args.files[1], args.threaded).unwrap();
            audio_comparator
                .run(
                    args.write_result,
                    opening_search_percentage,
                    Duration::from_secs(args.min_opening_duration as u64),
                    Duration::from_secs(args.min_ending_duration as u64),
                )
                .unwrap();
        }
        #[cfg(feature = "video")]
        Mode::Video => {
            let mut video_comparator =
                video::VideoComparator::new(&args.files[0], &args.files[1]).unwrap();
            video_comparator.compare(1000).unwrap();
        }
    }
}
