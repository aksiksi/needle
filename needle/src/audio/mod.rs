mod analyzer;
mod comparator;
mod data;
mod util;

pub use analyzer::Analyzer;
pub use data::FrameHashes;
pub use comparator::{Comparator, SearchResult};

/// Default hash match threshold.
///
/// This is used to determine if two frame hashes match. The value of a frame hash ranges
/// from 0 (exact match) to 32 (no match).
pub const DEFAULT_HASH_MATCH_THRESHOLD: u16 = 10;

/// Default opening search percentage.
///
/// If a match is found in the first percentage of the video, it is considered as an opening.
pub const DEFAULT_OPENING_SEARCH_PERCENTAGE: f32 = 0.50;

/// Default ending search percentage.
///
/// If a match is found in the last percentage of the video, it is considered as an ending.
pub const DEFAULT_ENDING_SEARCH_PERCENTAGE: f32 = 0.25;

/// Default minimum opening duration (seconds).
///
/// A match will only be considered as an opening if it runs for at least this long.
pub const DEFAULT_MIN_OPENING_DURATION: u16 = 20; // seconds

/// Default minimum ending duration (seconds).
///
/// A match will only be considered as an ending if it runs for at least this long.
pub const DEFAULT_MIN_ENDING_DURATION: u16 = 20; // seconds

/// Default hash period (seconds).
///
/// This is the time (in seconds) between successive frame hashes.
pub const DEFAULT_HASH_PERIOD: f32 = 0.3;

/// Default hash duration (seconds).
///
/// This is the duration of audio used to generate each frame hash. The minimum is 3 seconds -
/// this is a constraint imposed by the underlying audio fingerprinting algorithm, Chromaprint.
pub const DEFAULT_HASH_DURATION: f32 = 3.0;

/// Default opening and ending time padding (seconds).
///
/// This amount is added to the start time and subtracted from the end time of each opening and ending.
/// The idea is to provide a buffer that reduces the amount of missed content.
pub const DEFAULT_OPENING_AND_ENDING_TIME_PADDING: f32 = 0.0; // seconds

static FRAME_HASH_DATA_FILE_EXT: &str = "needle.bin";
static SKIP_FILE_EXT: &str = "needle.skip.json";
