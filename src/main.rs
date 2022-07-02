use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgAction, CommandFactory, ErrorKind, Parser, Subcommand};
#[cfg(feature = "rayon")]
use rayon::prelude::*;

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
    #[error("frame hash data not found at: {0:?}")]
    FrameHashDataNotFound(PathBuf),
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[clap(
        arg_required_else_help = true,
        after_help = "Decode one or more video files into a list of frame hashes. The frame hash data is written to disk alongside each analyzed video file, and is used by the 'search' command."
    )]
    Analyze {
        #[clap(
            required = true,
            multiple_values = true,
            help = "Video files or directories to analyze."
        )]
        paths: Vec<PathBuf>,

        #[clap(short, long, value_enum, default_value_t = Mode::Audio, help = "Analysis mode. The default mode is audio, which uses audio streams to find potential openings and endings. Video mode is less accurate and _much_ slower, but is useful if no audio stream is available.")]
        mode: Mode,

        #[clap(
            long,
            default_value_t = audio::Analyzer::DEFAULT_HASH_PERIOD,
            value_parser = clap::value_parser!(f32),
            help = "Period between hashes, in seconds. For example, if set to 0.3, a hash will be generated for every 300 ms of audio. Lowering this number can improve the accuracy of the result, at the cost of performance. The default aims to strike a balance between accuracy and performance."
        )]
        hash_period: f32,

        #[clap(
            long,
            default_value_t = audio::Analyzer::DEFAULT_HASH_DURATION,
            value_parser = clap::value_parser!(f32),
            help = "Duration of audio to hash, in seconds.",
        )]
        hash_duration: f32,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "Enable multi-threaded decoding in ffmpeg."
        )]
        threaded_decoding: bool,
    },
    #[clap(
        arg_required_else_help = true,
        after_help = "Search for openings and endings among a group of videos using frame hash data. Hash data can either be pre-computed and stored alongside video files using the 'analyze' command, or generated as part of the search by specifying the --analyze flag. Note that precomputation saves a ton of time."
    )]
    Search {
        #[clap(
            required = true,
            multiple_values = true,
            help = "Video files or directories to search for openings and endings in."
        )]
        paths: Vec<PathBuf>,

        #[clap(
            long,
            default_value_t = audio::Comparator::DEFAULT_HASH_MATCH_THRESHOLD,
            value_parser = clap::value_parser!(u16),
            help = "Threshold to use when comparing hashes. The range is 0 (exact match) to 32 (no match).",
        )]
        hash_match_threshold: u16,

        #[clap(
            long,
            default_value_t = audio::Comparator::DEFAULT_OPENING_SEARCH_PERCENTAGE,
            value_parser = clap::value_parser!(f32),
            help = "Specifies which portion of the video the opening and ending should be in. For example, if set to 0.75, a match found in the first 75% of the video will be considered the opening, while a match in the last 25% will be considered the ending."
        )]
        opening_search_percentage: f32,

        #[clap(
            long,
            default_value_t = audio::Comparator::DEFAULT_MIN_OPENING_DURATION,
            value_parser = clap::value_parser!(u16),
            help = "Minimum opening duration, in seconds. Setting a value that is close to the actual length helps reduce false positives (i.e., detecting an opening when there isn't one)."
        )]
        min_opening_duration: u16,

        #[clap(
            long,
            default_value_t = audio::Comparator::DEFAULT_MIN_ENDING_DURATION,
            value_parser = clap::value_parser!(u16),
            help = "Minimum ending duration, in seconds. Setting a value that is close to the actual length helps reduce false positives (i.e., detecting an ending when there isn't one)."
        )]
        min_ending_duration: u16,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "Run the analysis step in-place instead of looking for pre-computed hash data."
        )]
        analyze: bool,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "Create skip files. These are JSON files that store the result of the search alongside each video file."
        )]
        create_skip_files: bool,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "Do not display results of the search in stdout."
        )]
        no_display: bool,
    },
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

impl Cli {
    fn validate(&self) {
        let mut cmd = Cli::command();
        match self.command {
            Commands::Analyze {
                hash_period,
                hash_duration,
                ..
            } => {
                if hash_period <= 0.0 {
                    cmd.error(
                        ErrorKind::InvalidValue,
                        "hash_period must be a positive number",
                    )
                    .exit();
                }
                if hash_duration < 3.0 {
                    cmd.error(
                        ErrorKind::InvalidValue,
                        "hash_duration must be greater than 3 seconds",
                    )
                    .exit();
                }
            }
            Commands::Search {
                hash_match_threshold,
                opening_search_percentage,
                ..
            } => {
                if opening_search_percentage >= 1.0 {
                    cmd.error(
                        ErrorKind::InvalidValue,
                        "opening_search_percentage must be less than 1.0",
                    )
                    .exit();
                }
                if hash_match_threshold > 32 {
                    cmd.error(
                        ErrorKind::InvalidValue,
                        "hash_match_threshold cannot be larger than 32",
                    )
                    .exit();
                }
            }
        }
    }

    fn videos(&self) -> &Vec<PathBuf> {
        match self.command {
            Commands::Analyze { ref paths, .. } => paths,
            Commands::Search { ref paths, .. } => paths,
        }
    }

    fn find_video_files(&self) -> Vec<PathBuf> {
        // Validate all paths.
        for path in self.videos() {
            if !path.exists() {
                let mut cmd = Cli::command();
                cmd.error(
                    ErrorKind::InvalidValue,
                    format!("path not found: {}", path.display()),
                )
                .exit();
            }
        }

        // Find valid video files.
        let mut valid_video_files = Vec::new();
        for path in self.videos() {
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

        valid_video_files
    }
}

fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    ffmpeg_next::init().unwrap();

    let args = Cli::parse();
    args.validate();
    let video_files = args.find_video_files();

    match args.command {
        Commands::Analyze {
            mode,
            hash_period,
            hash_duration,
            threaded_decoding,
            ..
        } => match mode {
            Mode::Audio => {
                // Generate a list of analyzers, one per input video file.
                let mut analyzers = Vec::new();
                for video in video_files {
                    analyzers.push(audio::Analyzer::new(&video, threaded_decoding)?);
                }

                #[cfg(feature = "rayon")]
                analyzers.par_iter().for_each(|analyzer| {
                    analyzer.run(hash_period, hash_duration, true).unwrap();
                });
                #[cfg(not(feature = "rayon"))]
                analyzers.iter().for_each(|analyzer| {
                    analyzer.run(hash_period, hash_duration, true).unwrap();
                });
            }
            #[cfg(feature = "video")]
            Mode::Video => {
                let mut video_comparator =
                    video::VideoComparator::new(&args.files[0], &args.files[1]).unwrap();
                video_comparator.compare(1000).unwrap();
            }
        },
        Commands::Search {
            hash_match_threshold,
            opening_search_percentage,
            min_opening_duration,
            min_ending_duration,
            analyze,
            no_display,
            create_skip_files,
            ..
        } => {
            if video_files.len() < 2 {
                let mut cmd = Cli::command();
                cmd.error(
                    ErrorKind::InvalidValue,
                    format!(
                    "need at least 2 valid video files, but only found {} in provided video paths",
                    video_files.len()
                ),
                )
                .exit();
            }
            let min_opening_duration = Duration::from_secs(min_opening_duration.into());
            let min_ending_duration = Duration::from_secs(min_ending_duration.into());

            // Build a list of comparators by generating all unique pairs of videos from the set of provided
            // video file paths. Given N paths, this would result in: (N * (N-1)) / 2 comparators.
            let mut comparators = Vec::new();
            let mut processed_videos = HashSet::new();
            for (i, v1) in video_files.iter().enumerate() {
                for (j, v2) in video_files.iter().enumerate() {
                    if i == j || processed_videos.contains(v2) {
                        continue;
                    }
                    let c = audio::Comparator::from_files(
                        v1,
                        v2,
                        hash_match_threshold,
                        opening_search_percentage,
                        min_opening_duration,
                        min_ending_duration,
                    )?;
                    comparators.push(c);
                }
                processed_videos.insert(v1);
            }

            #[cfg(feature = "rayon")]
            comparators.par_iter().for_each(|c| {
                c.run(analyze, !no_display, create_skip_files).unwrap();
            });
            #[cfg(not(feature = "rayon"))]
            comparators.iter().for_each(|c| {
                c.run(analyze, !no_display, create_skip_files).unwrap();
            });
        }
    }

    Ok(())
}
