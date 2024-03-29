use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgAction, CommandFactory, ErrorKind, Parser, Subcommand};

use needle::audio;

#[derive(clap::ValueEnum, Clone, Debug)]
enum Mode {
    Audio,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[clap(after_help = "Displays info about needle and its dependencies.")]
    Info,

    #[clap(
        arg_required_else_help = true,
        after_help = "Decode one or more video files into a list of frame hashes. The frame hash data is written to disk alongside each analyzed video file, and is used by the 'search' command."
    )]
    Analyze {
        #[clap(
            required = true,
            multiple_values = true,
            value_parser = clap::value_parser!(PathBuf),
            help = "Video files or directories to analyze."
        )]
        paths: Vec<PathBuf>,

        #[clap(short, long, value_enum, default_value_t = Mode::Audio, help = "Analysis mode. The default mode is audio, which uses audio streams to find potential openings and endings. Video mode is less accurate and _much_ slower, but is useful if no audio stream is available.")]
        mode: Mode,

        #[clap(
            long,
            default_value_t = audio::DEFAULT_OPENING_SEARCH_PERCENTAGE,
            value_parser = clap::value_parser!(f32),
            help = "Specifies which portion of the start of the video the opening should be in. For example, if set to 0.25, only matches found in the first 25% of the video will be considered."
        )]
        opening_search_percentage: f32,

        #[clap(
            long,
            default_value_t = audio::DEFAULT_ENDING_SEARCH_PERCENTAGE,
            value_parser = clap::value_parser!(f32),
            help = "Specifies which portion of the end of the video the ending should be in. For example, if set to 0.25, only matches found in the last 25% of the video will be considered. Note: this is only relevant with --include_endings."
        )]
        ending_search_percentage: f32,

        #[clap(
            long,
            default_value_t = audio::DEFAULT_HASH_DURATION,
            value_parser = clap::value_parser!(f32),
            help = "Amount of time (in seconds) that each hash represents. The default should be sufficient for the vast majority of cases."
        )]
        hash_duration: f32,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "If set, needle will also consider endings during the analysis."
        )]
        include_endings: bool,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "Enable multi-threaded decoding in FFmpeg."
        )]
        threaded_decoding: bool,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "Re-analyze all videos and ignore any existing hash data on disk."
        )]
        force: bool,
    },

    #[clap(
        arg_required_else_help = true,
        after_help = "Search for openings and endings among a group of videos using frame hash data. Hash data can either be pre-computed and stored alongside video files using the 'analyze' command, or generated as part of the search by specifying the --analyze flag. Note that precomputation saves a ton of time."
    )]
    Search {
        #[clap(
            required = true,
            multiple_values = true,
            value_parser = clap::value_parser!(PathBuf),
            help = "Video files or directories to search for openings and endings in."
        )]
        paths: Vec<PathBuf>,

        #[clap(
            long,
            default_value_t = audio::DEFAULT_HASH_MATCH_THRESHOLD,
            value_parser = clap::value_parser!(u16),
            help = "Threshold to use when comparing hashes. The range is 0 (exact match) to 32 (no match).",
        )]
        hash_match_threshold: u16,

        #[clap(
            long,
            default_value_t = audio::DEFAULT_MIN_OPENING_DURATION,
            value_parser = clap::value_parser!(u16),
            help = "Minimum opening duration, in seconds. Setting a value that is close to the actual length helps reduce false positives (i.e., detecting an opening when there isn't one)."
        )]
        min_opening_duration: u16,

        #[clap(
            long,
            default_value_t = audio::DEFAULT_MIN_ENDING_DURATION,
            value_parser = clap::value_parser!(u16),
            help = "Minimum ending duration, in seconds. Note: this is only relevant with --include_endings."
        )]
        min_ending_duration: u16,

        #[clap(
            long,
            default_value_t = audio::DEFAULT_OPENING_AND_ENDING_TIME_PADDING,
            value_parser = clap::value_parser!(f32),
            help = "Amount of time (in seconds) to add to detected opening/ending start time and deduct from detected opening/ending end time."
        )]
        time_padding: f32,

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
            help = "Check skip files on disk during the search. These are JSON files that store the result of the search alongside each video file. When this flag is set, if a skip file exists for a video, it will be skipped during the pairwise search. Do not specify this flag if you want incremental search to work."
        )]
        use_skip_files: bool,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "Write skip files to disk after the search is completed. These are JSON files that store the result of the search alongside each video file. If skip files already exist for a video, it will be skipped during the search. This is central to how incremental search works."
        )]
        write_skip_files: bool,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "If set, needle will also consider endings during the search."
        )]
        include_endings: bool,

        #[clap(
            long,
            default_value = "false",
            action(ArgAction::SetTrue),
            help = "Do not display results of the search."
        )]
        no_display: bool,
    },
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,

    #[clap(
        long,
        global = true,
        default_value = "false",
        action(ArgAction::SetTrue),
        help = "Disable multi-threading."
    )]
    no_threading: bool,

    #[clap(
        long,
        global = true,
        default_value = "false",
        action(ArgAction::SetTrue),
        help = "By default, video files are validated using FFmpeg, which is extremely accurate. Setting this flag will switch to just checking file headers."
    )]
    file_headers_only: bool,
}

impl Cli {
    fn validate(&self) {
        let mut cmd = Cli::command();
        match self.command {
            Commands::Info => (),
            Commands::Analyze {
                opening_search_percentage,
                ending_search_percentage,
                hash_duration,
                ..
            } => {
                if opening_search_percentage >= 1.0 {
                    cmd.error(
                        ErrorKind::InvalidValue,
                        "opening_search_percentage must be less than 1.0",
                    )
                    .exit();
                }
                if ending_search_percentage >= 1.0 {
                    cmd.error(
                        ErrorKind::InvalidValue,
                        "ending_search_percentage must be less than 1.0",
                    )
                    .exit();
                }
                if hash_duration <= 0.0 {
                    cmd.error(
                        ErrorKind::InvalidValue,
                        "hash_duration must be greater than 0",
                    )
                    .exit();
                }
            }
            Commands::Search {
                hash_match_threshold,
                ..
            } => {
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

    fn find_video_files(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        match needle::util::find_video_files(paths, !self.file_headers_only, true) {
            Err(e) => {
                let mut cmd = Cli::command();
                cmd.error(ErrorKind::InvalidValue, e.to_string()).exit();
            }
            Ok(v) => v,
        }
    }
}

fn main() -> needle::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    ffmpeg_next::init().unwrap();

    // Set a high FFmpeg log level to reduce logging verbosity.
    ffmpeg_next::util::log::set_level(ffmpeg_next::util::log::Level::Fatal);

    let args = Cli::parse();
    args.validate();

    match args.command {
        Commands::Analyze {
            ref mode,
            opening_search_percentage,
            ending_search_percentage,
            hash_duration,
            include_endings,
            threaded_decoding,
            force,
            ref paths,
        } => match mode {
            Mode::Audio => {
                let mut videos = args.find_video_files(paths);
                videos.sort();
                let analyzer = audio::Analyzer::from_files(videos, threaded_decoding, force)
                    .with_opening_search_percentage(opening_search_percentage)
                    .with_ending_search_percentage(ending_search_percentage)
                    .with_include_endings(include_endings);
                let hash_duration = Duration::from_secs_f32(hash_duration);
                analyzer.run(hash_duration, true, !args.no_threading)?;
            }
        },
        Commands::Search {
            hash_match_threshold,
            min_opening_duration,
            min_ending_duration,
            analyze,
            no_display,
            include_endings,
            use_skip_files,
            write_skip_files,
            time_padding,
            ref paths,
        } => {
            let mut videos = args.find_video_files(paths);
            videos.sort();
            if videos.len() < 2 {
                let mut cmd = Cli::command();
                cmd.error(
                    ErrorKind::InvalidValue,
                    format!(
                    "need at least 2 valid video files, but only found {} in provided video paths",
                    paths.len()
                ),
                )
                .exit();
            }
            let min_opening_duration = Duration::from_secs(min_opening_duration.into());
            let min_ending_duration = Duration::from_secs(min_ending_duration.into());
            let time_padding = Duration::from_secs_f32(time_padding);
            let comparator = audio::Comparator::from_files(videos)
                .with_include_endings(include_endings)
                .with_hash_match_threshold(hash_match_threshold as u32)
                .with_min_opening_duration(min_opening_duration)
                .with_min_ending_duration(min_ending_duration)
                .with_time_padding(time_padding);
            comparator.run(
                analyze,
                !no_display,
                use_skip_files,
                write_skip_files,
                !args.no_threading,
            )?;
        }
        Commands::Info => {
            println!("FFmpeg version: {}", needle::util::ffmpeg_version_string());
        }
    }

    Ok(())
}
