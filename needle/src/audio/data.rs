use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Represents frame hash data for a single video file. This is the result of running
/// an [Analyzer] on a video file.
///
/// The struct contains the raw data as well as metadata about how the data was generated. The
/// original video size is included to allow for primitive duplicate checks when deciding whether
/// or not to skip analyzing a file.
#[derive(Debug, Deserialize, Serialize)]
pub struct FrameHashes {
    pub(crate) hash_period: f32,
    pub(crate) hash_duration: f32,
    pub(crate) data: Vec<(u32, Duration)>,
    pub(crate) md5: String,
}

impl FrameHashes {
    /// Load frame hashes from a path.
    fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::FrameHashDataNotFound(path.to_owned()).into());
        }
        let f = std::fs::File::open(path)?;
        Ok(bincode::deserialize_from(&f)?)
    }

    /// Load frame hash data using a video path.
    ///
    /// If `analyze` is set, the video is analyzed in-place. Otherwise, the frame data is
    /// loaded from alongside the video.
    pub fn from_video(video: impl AsRef<Path>, analyze: bool) -> Result<Self> {
        let video = video.as_ref();

        if !analyze {
            let path = video
                .to_owned()
                .with_extension(super::FRAME_HASH_DATA_FILE_EXT);
            Self::from_path(&path)
        } else {
            tracing::debug!(
                "starting in-place video analysis for {}...",
                video.display()
            );
            let analyzer = super::Analyzer::<&Path>::default().with_force(true);
            let frame_hashes = analyzer.run_single(
                video,
                super::DEFAULT_HASH_PERIOD,
                super::DEFAULT_HASH_DURATION,
                false,
            )?;
            tracing::debug!("completed in-place video analysis for {}", video.display());
            Ok(frame_hashes)
        }
    }
}
