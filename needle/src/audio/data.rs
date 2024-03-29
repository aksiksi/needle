use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct SkipFile {
    pub opening: Option<(f32, f32)>,
    pub ending: Option<(f32, f32)>,
    pub md5: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum FrameHashesVersion {
    V1 = 12345,
}

#[derive(Debug, Deserialize, Serialize)]
struct FrameHashesV1 {
    opening: Vec<(u32, Duration)>,
    ending: Vec<(u32, Duration)>,
    hash_duration: Duration,
    md5: String,
}

impl FrameHashesV1 {
    fn new(
        opening: Vec<(u32, Duration)>,
        ending: Vec<(u32, Duration)>,
        hash_duration: Duration,
        md5: String,
    ) -> Self {
        Self {
            opening,
            ending,
            hash_duration,
            md5,
        }
    }

    fn opening(&self) -> &[(u32, Duration)] {
        return &self.opening;
    }

    fn ending(&self) -> &[(u32, Duration)] {
        return &self.ending;
    }

    fn hash_duration(&self) -> Duration {
        return self.hash_duration;
    }

    fn md5(&self) -> &str {
        return &self.md5;
    }
}

#[derive(Debug, Deserialize, Serialize)]
enum FrameHashesData {
    // IMPORTANT: Removing and/or modifying any of these variants is a breaking change
    // to the on-disk binary format. To avoid this, just add a new format version
    // instead.
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
    pub(crate) fn new_v1(
        opening: Vec<(u32, Duration)>,
        ending: Vec<(u32, Duration)>,
        hash_duration: Duration,
        md5: String,
    ) -> Self {
        Self {
            version: FrameHashesVersion::V1,
            data: FrameHashesData::V1(FrameHashesV1::new(opening, ending, hash_duration, md5)),
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
                .with_extension(crate::FRAME_HASH_DATA_FILE_NAME);
            Self::from_path(&path)
        } else {
            tracing::debug!(
                "starting in-place video analysis for {}...",
                video.display()
            );
            let analyzer = super::Analyzer::<&Path>::default().with_force(true);
            let hash_duration = Duration::from_secs_f32(super::DEFAULT_HASH_DURATION);
            let frame_hashes = analyzer.run_single(video, hash_duration, false)?;
            tracing::debug!("completed in-place video analysis for {}", video.display());
            Ok(frame_hashes)
        }
    }

    /// Returns the data from the underlying frame hashes.
    pub fn opening_data(&self) -> &[(u32, Duration)] {
        match &self.data {
            FrameHashesData::V1(f) => f.opening(),
        }
    }

    /// Returns the data from the underlying frame hashes.
    pub fn ending_data(&self) -> &[(u32, Duration)] {
        match &self.data {
            FrameHashesData::V1(f) => f.ending(),
        }
    }

    /// Returns the hash duration from the underlying frame hashes.
    pub fn hash_duration(&self) -> Duration {
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
