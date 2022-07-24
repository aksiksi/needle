mod analyzer;
mod comparator;

pub use analyzer::{Analyzer, FrameHashes};
pub use comparator::Comparator;

pub const DEFAULT_HASH_MATCH_THRESHOLD: u16 = 15;
pub const DEFAULT_OPENING_SEARCH_PERCENTAGE: f32 = 0.33;
pub const DEFAULT_ENDING_SEARCH_PERCENTAGE: f32 = 0.25;
pub const DEFAULT_MIN_OPENING_DURATION: u16 = 20; // seconds
pub const DEFAULT_MIN_ENDING_DURATION: u16 = 20; // seconds
pub const DEFAULT_HASH_PERIOD: f32 = 0.3;
pub const DEFAULT_HASH_DURATION: f32 = 3.0;
pub const DEFAULT_OPENING_AND_ENDING_TIME_PADDING: f32 = 0.0; // seconds

static FRAME_HASH_DATA_FILE_EXT: &str = "needle.bin";
static SKIP_FILE_EXT: &str = "needle.skip.json";
