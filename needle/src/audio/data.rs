use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum FrameHashesVersion {
    V1 = 12345,
}

#[derive(Debug, Deserialize, Serialize)]
struct FrameHashesV1 {
    data: Vec<(u32, Duration)>,
    hash_duration: f32,
    md5: String,
}

impl FrameHashesV1 {
    fn new(data: Vec<(u32, Duration)>, hash_duration: f32, md5: String) -> Self {
        Self {
            data,
            hash_duration,
            md5,
        }
    }

    fn data(&self) -> &[(u32, Duration)] {
        return &self.data;
    }

    fn hash_duration(&self) -> f32 {
        return self.hash_duration;
    }

    fn md5(&self) -> &str {
        return &self.md5;
    }
}

#[derive(Debug, Deserialize, Serialize)]
enum FrameHashesData {
    // IMPORTANT: Removing or modifying any of these variants is a breaking change.
    V1(FrameHashesV1),
}

/// Represents frame hash data for a single video file. This is the result of running
/// an [Analyzer] on a video file.
///
/// The struct contains the raw data as well as metadata about how the data was generated.
/// The struct is versioned to allow for upgrades in the future without breaking previous
/// versions.
#[derive(Debug, Deserialize, Serialize)]
pub struct FrameHashes {
    /// Magic number for the version.
    pub version: FrameHashesVersion,
    /// Data for the given version.
    data: FrameHashesData,
}

impl FrameHashes {
    pub(crate) fn new_v1(data: Vec<(u32, Duration)>, hash_duration: f32, md5: String) -> Self {
        Self {
            version: FrameHashesVersion::V1,
            data: FrameHashesData::V1(FrameHashesV1::new(data, hash_duration, md5)),
        }
    }

    /// Ensures that the version magic number matches the version of the data.
    fn is_version_valid(&self) -> bool {
        match self.data {
            FrameHashesData::V1(_) if self.version == FrameHashesVersion::V1 => true,
            _ => false,
        }
    }

    /// Load frame hashes from a path.
    fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::FrameHashDataNotFound(path.to_owned()).into());
        }
        let f = std::fs::File::open(path)?;
        let frame_hashes: Self = bincode::deserialize_from(&f)?;
        if !frame_hashes.is_version_valid() {
            return Err(Error::FrameHashDataInvalidVersion);
        }
        Ok(frame_hashes)
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

    /// Returns the data from the underlying frame hashes.
    pub fn data(&self) -> &[(u32, Duration)] {
        match &self.data {
            FrameHashesData::V1(f) => f.data(),
        }
    }

    /// Returns the hash duration from the underlying frame hashes.
    pub fn hash_duration(&self) -> f32 {
        match &self.data {
            FrameHashesData::V1(f) => f.hash_duration(),
        }
    }

    /// Returns the MD5 hash from the underlying frame hashes.
    pub fn md5(&self) -> &str {
        match &self.data {
            FrameHashesData::V1(f) => f.md5(),
        }
    }
}
