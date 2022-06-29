use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgAction, CommandFactory, ErrorKind, Parser};

mod audio;
mod simhash;
mod util;
#[cfg(feature = "video")]
mod video;

#[derive(clap::ValueEnum, Clone, Debug)]
enum Mode {
    Audio,
    #[cfg(feature = "video")]
    Video,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid timestamp for seek: requested={requested:?} duration={duration:?}")]
    InvalidSeekTimestamp {
        requested: Duration,
        duration: Duration,
    },
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_enum, default_value_t = Mode::Audio, help = "Analysis mode. The default mode is audio, which uses audio streams to find potential openings and endings. Video mode is less accurate and _much_ slower, but is useful if no audio stream is available.")]
    mode: Mode,

    #[clap(required = true, help = "Video files or directories to analyze.")]
    videos: Vec<PathBuf>,

    #[clap(
        long,
        default_value = "0.3",
        value_parser = clap::value_parser!(f32),
        help = "Period between hashes, in seconds. For example, if set to 0.3, a hash will be generated for every 300 ms of audio. Lowering this number can improve the accuracy of the result, at the cost of performance. The default aims to strike a balance between accuracy and performance."
    )]
    hash_period: f32,

    #[clap(
        long,
        default_value = "3.0",
        value_parser = clap::value_parser!(f32),
        help = "Duration of audio to hash, in seconds.",
    )]
    hash_duration: f32,

    #[clap(
        long,
        default_value = "15",
        value_parser = clap::value_parser!(u16),
        help = "Threshold to use when comparing hashes. The range is 0 (exact match) to 32 (no match).",
    )]
    hash_match_threshold: u16,

    #[clap(
        long,
        default_value = "0.75",
        value_parser = clap::value_parser!(f32),
        help = "Specifies which portion of the video the opening and ending should be in. For example, if set to 0.75, a match found in the first 75% of the video will be considered the opening, while a match in the last 25% will be considered the ending."
    )]
    opening_search_percentage: f32,

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
        help = "Enable multi-threaded decoding in ffmpeg."
    )]
    threaded_decoding: bool,

    #[clap(
        short,
        long,
        default_value = "false",
        action(ArgAction::SetTrue),
        help = "Write detected raw audio clips to the current directory. Useful for debugging."
    )]
    write_result: bool,
}

fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    ffmpeg_next::init().unwrap();

    let args = Args::parse();
    let mut cmd = Args::command();

    if args.opening_search_percentage >= 1.0 {
        cmd.error(
            ErrorKind::InvalidValue,
            "opening_search_percentage must be less than 1.0",
        )
        .exit();
    }
    if args.hash_period <= 0.0 {
        cmd.error(
            ErrorKind::InvalidValue,
            "hash_period must be a positive number",
        )
        .exit();
    }
    if args.hash_duration < 3.0 {
        cmd.error(
            ErrorKind::InvalidValue,
            "hash_duration must be greater than 3 seconds",
        )
        .exit();
    }
    if args.hash_match_threshold > 32 {
        cmd.error(
            ErrorKind::InvalidValue,
            "hash_match_threshold cannot be larger than 32",
        )
        .exit();
    }

    // Validate all paths.
    for path in &args.videos {
        if !path.exists() {
            let mut cmd = Args::command();
            cmd.error(
                ErrorKind::InvalidValue,
                format!("path not found: {}", path.display()),
            )
            .exit();
        }
    }

    // Find valid video files.
    let mut valid_video_files = Vec::new();
    for path in &args.videos {
        if path.is_dir() {
            valid_video_files.extend(
                std::fs::read_dir(path)
                    .unwrap()
                    .map(|p| {
                        let entry = p.unwrap();
                        entry.path()
                    })
                    .filter(|p| util::is_valid_video_file(p, !cfg!(feature = "video")))
                    .collect::<Vec<_>>(),
            );
        } else {
            if util::is_valid_video_file(path, !cfg!(feature = "video")) {
                valid_video_files.push(path.clone());
            }
        }
    }

    if valid_video_files.len() < 2 {
        let mut cmd = Args::command();
        cmd.error(
            ErrorKind::InvalidValue,
            format!(
                "need at least 2 valid video files, but only found {} in provided video paths",
                valid_video_files.len()
            ),
        )
        .exit();
    }

    match args.mode {
        Mode::Audio => {
            let audio_comparator = audio::AudioComparator::new(
                &args.videos[0],
                &args.videos[1],
                args.hash_match_threshold,
                args.threaded_decoding,
            )
            .unwrap();
            audio_comparator
                .run(
                    args.hash_period,
                    args.hash_duration,
                    args.opening_search_percentage,
                    Duration::from_secs(args.min_opening_duration as u64),
                    Duration::from_secs(args.min_ending_duration as u64),
                    args.write_result,
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
