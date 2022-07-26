extern crate rayon;

use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chromaprint_rust as chromaprint;
#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::util;
use crate::Result;

use super::FrameHashes;

#[derive(serde::Deserialize, serde::Serialize)]
struct SkipFile {
    pub opening: Option<(f32, f32)>,
    pub ending: Option<(f32, f32)>,
    pub md5: String,
}

type ComparatorHeap = BinaryHeap<ComparatorHeapEntry>;

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct ComparatorHeapEntry {
    score: usize,
    src_longest_run: (Duration, Duration),
    dst_longest_run: (Duration, Duration),
    src_match_hash: u32,
    dst_match_hash: u32,
    is_src_opening: bool,
    is_dst_opening: bool,
    src_hash_duration: Duration,
    dst_hash_duration: Duration,
}

impl Display for ComparatorHeapEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "score: {}, src_longest_run: {:?}, dst_longest_run: {:?}",
            self.score, self.src_longest_run, self.dst_longest_run
        )
    }
}

#[derive(Debug)]
struct OpeningAndEndingInfo {
    src_openings: Vec<ComparatorHeapEntry>,
    dst_openings: Vec<ComparatorHeapEntry>,
    src_endings: Vec<ComparatorHeapEntry>,
    dst_endings: Vec<ComparatorHeapEntry>,
}

/// Represents a single result for a video file. This is output by [Comparator::run].
#[derive(Copy, Clone, Debug, Default)]
pub struct SearchResult {
    opening: Option<(Duration, Duration)>,
    ending: Option<(Duration, Duration)>,
}

/// Compares two or more video files using either existing [FrameHashes](super::FrameHashes) or by running an
/// [Analyzer](super::Analyzer) in-place.
#[derive(Debug)]
pub struct Comparator<P: AsRef<Path>> {
    videos: Vec<P>,
    hash_match_threshold: u32,
    opening_search_percentage: f32,
    ending_search_percentage: f32,
    min_opening_duration: Duration,
    min_ending_duration: Duration,
    time_padding: Duration,
}

impl<P: AsRef<Path>> Default for Comparator<P> {
    fn default() -> Self {
        Self {
            videos: Vec::new(),
            hash_match_threshold: super::DEFAULT_HASH_MATCH_THRESHOLD as u32,
            opening_search_percentage: super::DEFAULT_OPENING_SEARCH_PERCENTAGE,
            ending_search_percentage: super::DEFAULT_ENDING_SEARCH_PERCENTAGE,
            min_opening_duration: Duration::from_secs(super::DEFAULT_MIN_OPENING_DURATION as u64),
            min_ending_duration: Duration::from_secs(super::DEFAULT_MIN_ENDING_DURATION as u64),
            time_padding: Duration::ZERO,
        }
    }
}

impl<P: AsRef<Path>> Comparator<P> {
    /// Constructs a [Comparator] from a list of video paths.
    pub fn from_files(videos: impl Into<Vec<P>>) -> Self {
        let mut comparator = Self::default();
        comparator.videos = videos.into();
        comparator
    }

    /// Returns the video paths used by this comparator.
    pub fn videos(&self) -> &[P] {
        &self.videos
    }

    /// Returns a new [Comparator] with the provided `hash_match_threshold`.
    pub fn with_hash_match_threshold(mut self, hash_match_threshold: u32) -> Self {
        self.hash_match_threshold = hash_match_threshold;
        self
    }

    /// Returns a new [Comparator] with the provided `opening_search_percentage`.
    pub fn with_opening_search_percentage(mut self, opening_search_percentage: f32) -> Self {
        self.opening_search_percentage = opening_search_percentage;
        self
    }

    /// Returns a new [Comparator] with the provided `ending_search_percentage`.
    pub fn with_ending_search_percentage(mut self, ending_search_percentage: f32) -> Self {
        self.ending_search_percentage = ending_search_percentage;
        self
    }

    /// Returns a new [Comparator] with the provided `min_opening_duration`.
    pub fn with_min_opening_duration(mut self, min_opening_duration: Duration) -> Self {
        self.min_opening_duration = min_opening_duration;
        self
    }

    /// Returns a new [Comparator] with the provided `min_ending_duration`.
    pub fn with_min_ending_duration(mut self, min_ending_duration: Duration) -> Self {
        self.min_ending_duration = min_ending_duration;
        self
    }

    /// Returns a new [Comparator] with the provided `time_padding`.
    pub fn with_time_padding(mut self, time_padding: Duration) -> Self {
        self.time_padding = time_padding;
        self
    }

    #[inline]
    fn compute_hash_for_match(hashes: &[(u32, Duration)], (start, end): (usize, usize)) -> u32 {
        let hashes: Vec<u32> = hashes.iter().map(|t| t.0).collect();
        chromaprint::simhash::simhash32(&hashes[start..end + 1])
    }

    /// Runs a LCS (longest common substring) search between the two sets of hashes. This runs in
    /// O(n * m) time.
    fn longest_common_hash_match(
        &self,
        src: &[(u32, Duration)],
        dst: &[(u32, Duration)],
        src_max_opening_time: Duration,
        src_min_ending_time: Duration,
        dst_max_opening_time: Duration,
        dst_min_ending_time: Duration,
        src_hash_duration: Duration,
        dst_hash_duration: Duration,
        heap: &mut ComparatorHeap,
    ) {
        // Build the DP table of substrings.
        let mut table: Vec<Vec<usize>> = vec![vec![0; dst.len() + 1]; src.len() + 1];
        for i in 0..src.len() {
            for j in 0..dst.len() {
                let (src_hash, dst_hash) = (src[i].0, dst[j].0);
                if i == 0 || j == 0 {
                    table[i][j] = 0;
                } else if u32::count_ones(src_hash ^ dst_hash) <= self.hash_match_threshold {
                    table[i][j] = table[i - 1][j - 1] + 1;
                } else {
                    table[i][j] = 0;
                }
            }
        }

        // Walk through the table and find all valid substrings and insert them into
        // the heap.
        let mut i = src.len() - 1;
        while i > 0 {
            let mut j = dst.len() - 1;
            while j > 0 {
                // We need to find an entry where the current entry is non-zero
                // and the next entry is zero. This indicates that we are at the end
                // of a substring.
                if table[i][j] == 0
                    || (i < src.len() - 1 && j < dst.len() - 1 && table[i + 1][j + 1] != 0)
                {
                    j -= 1;
                    continue;
                }

                let (src_start_idx, src_end_idx) = (i - table[i][j], i);
                let (dst_start_idx, dst_end_idx) = (j - table[i][j], j);

                let (src_start, src_end) = (src[src_start_idx].1, src[src_end_idx].1);
                let (dst_start, dst_end) = (dst[dst_start_idx].1, dst[dst_end_idx].1);
                let (is_src_opening, is_src_ending) = (
                    src_end < src_max_opening_time,
                    src_start > src_min_ending_time,
                );
                let (is_dst_opening, is_dst_ending) = (
                    dst_end < dst_max_opening_time,
                    dst_start > dst_min_ending_time,
                );

                let is_valid = (is_src_opening
                    && (src_end - src_start) >= self.min_opening_duration)
                    || (is_src_ending && (src_end - src_start) >= self.min_ending_duration)
                    || (is_dst_opening && (dst_end - dst_start) >= self.min_opening_duration)
                    || (is_dst_ending && (dst_end - dst_start) >= self.min_ending_duration);

                if is_valid {
                    let src_match_hash =
                        Self::compute_hash_for_match(src, (src_start_idx, src_end_idx));
                    let dst_match_hash =
                        Self::compute_hash_for_match(dst, (dst_start_idx, dst_end_idx));

                    let entry = ComparatorHeapEntry {
                        score: table[i][j],
                        src_longest_run: (src_start, src_end),
                        dst_longest_run: (dst_start, dst_end),
                        src_match_hash,
                        dst_match_hash,
                        is_src_opening,
                        is_dst_opening,
                        src_hash_duration,
                        dst_hash_duration,
                    };

                    heap.push(entry);
                }

                j -= 1;
            }

            i -= 1;
        }
    }

    fn find_opening_and_ending(
        &self,
        src_hashes: &super::analyzer::FrameHashes,
        dst_hashes: &super::analyzer::FrameHashes,
    ) -> OpeningAndEndingInfo {
        let _g = tracing::span!(tracing::Level::TRACE, "find_opening_and_ending");

        let src_hash_data = &src_hashes.data;
        let dst_hash_data = &dst_hashes.data;
        let src_hash_duration = Duration::from_secs_f32(src_hashes.hash_duration);
        let dst_hash_duration = Duration::from_secs_f32(dst_hashes.hash_duration);

        let mut heap: ComparatorHeap =
            BinaryHeap::with_capacity(src_hash_data.len() + dst_hash_data.len());

        // Figure out the duration limits for opening and endings.
        let src_opening_search_idx =
            ((src_hash_data.len() - 1) as f32 * self.opening_search_percentage) as usize;
        let src_ending_search_idx =
            ((src_hash_data.len() - 1) as f32 * (1.0 - self.ending_search_percentage)) as usize;
        let dst_opening_search_idx =
            ((dst_hash_data.len() - 1) as f32 * self.opening_search_percentage) as usize;
        let dst_ending_search_idx =
            ((dst_hash_data.len() - 1) as f32 * (1.0 - self.ending_search_percentage)) as usize;
        let src_max_opening_time = src_hash_data[src_opening_search_idx].1;
        let src_min_ending_time = src_hash_data[src_ending_search_idx].1;
        let dst_max_opening_time = dst_hash_data[dst_opening_search_idx].1;
        let dst_min_ending_time = dst_hash_data[dst_ending_search_idx].1;

        self.longest_common_hash_match(
            src_hash_data,
            dst_hash_data,
            src_max_opening_time,
            src_min_ending_time,
            dst_max_opening_time,
            dst_min_ending_time,
            src_hash_duration,
            dst_hash_duration,
            &mut heap,
        );

        tracing::debug!(heap_size = heap.len(), "finished sliding window analysis");

        let (mut src_valid_openings, mut src_valid_endings) = (Vec::new(), Vec::new());
        let (mut dst_valid_openings, mut dst_valid_endings) = (Vec::new(), Vec::new());

        while let Some(entry) = heap.pop() {
            let (is_src_opening, is_dst_opening) = (entry.is_src_opening, entry.is_dst_opening);
            if is_src_opening {
                src_valid_openings.push(entry.clone());
            } else {
                src_valid_endings.push(entry.clone());
            }
            if is_dst_opening {
                dst_valid_openings.push(entry.clone());
            } else {
                dst_valid_endings.push(entry.clone());
            }
        }

        OpeningAndEndingInfo {
            src_openings: src_valid_openings,
            dst_openings: dst_valid_openings,
            src_endings: src_valid_endings,
            dst_endings: dst_valid_endings,
        }
    }

    fn check_skip_file(video: impl AsRef<Path>) -> Result<bool> {
        let skip_file = video
            .as_ref()
            .to_owned()
            .with_extension(super::SKIP_FILE_EXT);
        if !skip_file.exists() {
            return Ok(false);
        }

        // Compute MD5 hash of the video header.
        let md5 = crate::util::compute_header_md5sum(video)?;

        // Read existing skip file and compare MD5 hashes.
        let f = std::fs::File::open(&skip_file)?;
        let skip_file: SkipFile = serde_json::from_reader(&f).unwrap();

        Ok(skip_file.md5 == md5)
    }

    fn create_skip_file(&self, video: impl AsRef<Path>, result: SearchResult) -> Result<()> {
        let opening = result
            .opening
            .map(|(start, end)| (start.as_secs_f32(), end.as_secs_f32()));
        let ending = result
            .ending
            .map(|(start, end)| (start.as_secs_f32(), end.as_secs_f32()));
        if opening.is_none() && ending.is_none() {
            return Ok(());
        }

        let md5 = crate::util::compute_header_md5sum(&video)?;
        let skip_file = video
            .as_ref()
            .to_owned()
            .with_extension(super::SKIP_FILE_EXT);
        let mut skip_file = std::fs::File::create(skip_file)?;
        let data = SkipFile {
            opening,
            ending,
            md5,
        };
        serde_json::to_writer(&mut skip_file, &data)?;

        Ok(())
    }

    fn display_opening_ending_info(&self, path: &Path, result: SearchResult) {
        println!("\n{}\n", path.display());
        if let Some(opening) = result.opening {
            let (start, end) = opening;
            println!(
                "* Opening - {:?}-{:?}",
                util::format_time(start),
                util::format_time(end)
            );
        } else {
            println!("* Opening - N/A");
        }
        if let Some(ending) = result.ending {
            let (start, end) = ending;
            println!(
                "* Ending - {:?}-{:?}",
                util::format_time(start),
                util::format_time(end)
            );
        } else {
            println!("* Ending - N/A");
        }
    }

    fn search(
        &self,
        src_path: &Path,
        dst_path: &Path,
        frame_hash_map: &HashMap<&Path, FrameHashes>,
    ) -> Result<OpeningAndEndingInfo> {
        tracing::debug!("started audio comparator");

        let (src_frame_hashes, dst_frame_hashes) = {
            (
                frame_hash_map
                    .get(src_path)
                    .expect("frame hash data not found for src_path"),
                frame_hash_map
                    .get(dst_path)
                    .expect("frame hash data not found for dst_path"),
            )
        };

        tracing::debug!("starting search for opening and ending");
        let info = self.find_opening_and_ending(src_frame_hashes, dst_frame_hashes);
        tracing::debug!("finished search for opening and ending");

        Ok(info)
    }

    /// Find the best opening and ending candidate across all provided matches.
    ///
    /// The idea is simple: keep track of the longest opening and ending detected among all of the matches
    /// and combine them to determine the best overall match.
    fn find_best_match(&self, matches: &[(&OpeningAndEndingInfo, bool)]) -> Option<SearchResult> {
        // TODO(aksiksi): Use the number of distinct matches along with duration. For example, it could be that the longest
        // opening was not actually the opening but instead a montage song found in one or two other episodes.
        if matches.len() == 0 {
            return None;
        }

        let mut result: SearchResult = Default::default();
        let mut best_opening_duration = Duration::ZERO;
        let mut best_ending_duration = Duration::ZERO;

        for (m, is_source) in matches {
            let opening;
            let ending;
            if *is_source {
                opening = m
                    .src_openings
                    .first()
                    .map(|e| (e.src_longest_run, e.src_hash_duration));
                ending = m
                    .src_endings
                    .first()
                    .map(|e| (e.src_longest_run, e.src_hash_duration));
            } else {
                opening = m
                    .dst_openings
                    .first()
                    .map(|e| (e.dst_longest_run, e.dst_hash_duration));
                ending = m
                    .dst_endings
                    .first()
                    .map(|e| (e.dst_longest_run, e.dst_hash_duration));
            }

            if let Some(((start, end), hash_duration)) = opening {
                let duration = end - start;
                if duration >= best_opening_duration {
                    result.opening = Some((
                        // Add a buffer between actual detected times and what we return to users.
                        start + self.time_padding,
                        // Adjust ending time using the configured hash duration.
                        end - self.time_padding - hash_duration,
                    ));
                    best_opening_duration = duration;
                }
            }
            if let Some(((start, end), hash_duration)) = ending {
                let duration = end - start;
                if duration >= best_ending_duration {
                    result.ending = Some((
                        // Add a buffer between actual detected times and what we return to users.
                        start + self.time_padding,
                        // Adjust ending time using the configured hash duration.
                        end - self.time_padding - hash_duration,
                    ));
                    best_ending_duration = duration;
                }
            }
        }

        Some(result)
    }
}

impl<P: AsRef<Path> + Sync> Comparator<P> {
    /// Runs the comparator.
    ///
    /// If `analyze` is set to true, an `Analyzer` will be built for each video file and run in-place.
    /// If `display` is set, the final results will be printed to stdout. If `use_skip_files` is set,
    /// if a skip file already exists for a video, the video will be skipped during this run. If `write_skip_files`
    /// is set, a skip file will be written to disk once the comparator is completed.
    pub fn run(
        &self,
        analyze: bool,
        display: bool,
        use_skip_files: bool,
        write_skip_files: bool,
    ) -> Result<BTreeMap<PathBuf, SearchResult>> {
        // Build a list of video pairs for actual search. Pairs should only appear once.
        // Given N videos, this will result in: (N * (N-1)) / 2 pairs
        let mut pairs = Vec::new();
        let mut processed_videos = HashSet::new();

        // Stores frame hash data for each video to be analyzed.
        // We load them all now to be able to handle in-place analysis when the `analyze`
        // flag is passed in to this method.
        let mut frame_hash_map = HashMap::new();

        for (i, v1) in self.videos.iter().enumerate() {
            let v1 = v1.as_ref();

            // Skip processing this video if it already has a skip file on disk.
            // TODO(aksiksi): This needs to be fixed.
            if use_skip_files && Self::check_skip_file(v1)? {
                println!("Skipping {} due to existing skip file...", v1.display());
                processed_videos.insert(v1);
                continue;
            }

            let frame_hashes = if analyze {
                // If analysis is required, run the analysis in-place even if data already exists.
                tracing::debug!("starting in-place video analysis for {}...", v1.display());
                let analyzer = super::Analyzer::<P>::default().with_force(true);
                let frame_hashes = analyzer.run_single(
                    v1,
                    super::DEFAULT_HASH_PERIOD,
                    super::DEFAULT_HASH_DURATION,
                    false,
                )?;
                tracing::debug!("completed in-place video analysis for {}", v1.display());
                frame_hashes
            } else {
                // Otherwise, load the frame hash data from disk.
                FrameHashes::from_video(v1)?
            };

            frame_hash_map.insert(v1, frame_hashes);

            for (j, v2) in self.videos.iter().enumerate() {
                let v2 = v2.as_ref();
                if i == j || processed_videos.contains(v2) {
                    continue;
                }
                pairs.push((v1, v2));
            }
            processed_videos.insert(v1);
        }

        // Perform the search in parallel for all pairs.
        #[cfg(feature = "rayon")]
        let data = pairs
            .par_iter()
            .map(|(src_path, dst_path)| {
                (
                    *src_path,
                    *dst_path,
                    self.search(src_path, dst_path, &frame_hash_map).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        #[cfg(not(feature = "rayon"))]
        let data = pairs
            .iter()
            .map(|(src_path, dst_path)| {
                (
                    *src_path,
                    *dst_path,
                    self.search(src_path, dst_path, analyze).unwrap(),
                )
            })
            .collect::<Vec<_>>();

        // This map tracks the generated info struct for each video path. A bool is included
        // to allow determining whether the path is a source (true) or dest (false) in the info
        // struct.
        let mut info_map: BTreeMap<&Path, Vec<(&OpeningAndEndingInfo, bool)>> = BTreeMap::new();
        for (src_path, dst_path, info) in &data {
            if let Some(v) = info_map.get_mut(*src_path) {
                v.push((info, true));
            } else {
                info_map.insert(*src_path, vec![(info, true)]);
            }
            if let Some(v) = info_map.get_mut(*dst_path) {
                v.push((info, false));
            } else {
                info_map.insert(*dst_path, vec![(info, false)]);
            }
        }

        let mut match_map = BTreeMap::new();

        // For each path, find the best opening and ending candidate among the list
        // of other videos. If required, display the result and write a skip file to disk.
        for (path, matches) in info_map {
            let result = self.find_best_match(&matches);
            if result.is_none() {
                println!("No opening or ending found for: {}", path.display());
                continue;
            }
            let result = result.unwrap();
            if display {
                self.display_opening_ending_info(path, result);
            }
            if write_skip_files {
                self.create_skip_file(path, result)?;
            }

            match_map.insert(path.to_owned(), result.clone());
        }

        Ok(match_map)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_sample_paths() -> Vec<PathBuf> {
        let resources = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        vec![
            resources.join("sample-5s.mp4"),
            resources.join("sample-shifted-4s.mp4"),
        ]
    }

    #[test]
    fn test_comparator() {
        let paths = get_sample_paths();
        let comparator = Comparator::from_files(paths);
        let data = comparator.run(true, true, false, false).unwrap();
        assert_eq!(data.len(), 2);
    }
}
