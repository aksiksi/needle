#[cfg(feature = "rayon")]
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
    is_src_ending: bool,
    is_dst_opening: bool,
    is_dst_ending: bool,
    src_hash_duration: Duration,
    dst_hash_duration: Duration,
}

impl Display for ComparatorHeapEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "score: {}, src_longest_run: {:?}, dst_longest_run: {:?}, src_match_hash: {}, dst_match_hash: {}",
            self.score, self.src_longest_run, self.dst_longest_run, self.src_match_hash, self.dst_match_hash,
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

impl OpeningAndEndingInfo {
    pub fn is_empty(&self) -> bool {
        self.src_openings.is_empty()
            && self.dst_openings.is_empty()
            && self.src_endings.is_empty()
            && self.dst_endings.is_empty()
    }
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

impl<P: AsRef<Path> + Ord> Comparator<P> {
    /// Constructs a [Comparator] from a list of video paths.
    pub fn from_files(videos: impl Into<Vec<P>>) -> Self {
        let mut comparator = Self::default();
        comparator.videos = videos.into();
        comparator.videos.sort();
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
    ) -> Vec<ComparatorHeapEntry> {
        // Heap to keep track of best hash matches in order of length.
        let mut heap: ComparatorHeap = BinaryHeap::new();

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

                // Figure out whether this is an opening or an ending.
                //
                // If the sequence _ends_ before the maximum opening time, it is an opening.
                // If the sequence _starts_ after the maximum ending time, it is an ending.
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

                // A LCS result is only valid iff it is a valid opening or ending in the source _and_ the dest.
                let is_src_valid = (is_src_opening
                    && (src_end - src_start) >= self.min_opening_duration)
                    || (is_src_ending && (src_end - src_start) >= self.min_ending_duration);
                let is_dst_valid = (is_dst_opening
                    && (dst_end - dst_start) >= self.min_opening_duration)
                    || (is_dst_ending && (dst_end - dst_start) >= self.min_ending_duration);
                let is_valid = is_src_valid && is_dst_valid;

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
                        is_src_ending,
                        is_dst_opening,
                        is_dst_ending,
                        src_hash_duration,
                        dst_hash_duration,
                    };

                    heap.push(entry);
                }

                j -= 1;
            }

            i -= 1;
        }

        heap.into()
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

        let entries = self.longest_common_hash_match(
            src_hash_data,
            dst_hash_data,
            src_max_opening_time,
            src_min_ending_time,
            dst_max_opening_time,
            dst_min_ending_time,
            src_hash_duration,
            dst_hash_duration,
        );

        tracing::debug!(
            num_matches = entries.len(),
            "finished sliding window analysis"
        );

        let (mut src_valid_openings, mut src_valid_endings) = (Vec::new(), Vec::new());
        let (mut dst_valid_openings, mut dst_valid_endings) = (Vec::new(), Vec::new());

        // TODO(aksiksi): Reduce duplication of memory here.
        for entry in entries {
            let (is_src_opening, is_src_ending) = (entry.is_src_opening, entry.is_src_ending);
            let (is_dst_opening, is_dst_ending) = (entry.is_dst_opening, entry.is_dst_ending);
            if is_src_opening {
                src_valid_openings.push(entry.clone());
            } else if is_src_ending {
                src_valid_endings.push(entry.clone());
            }
            if is_dst_opening {
                dst_valid_openings.push(entry.clone());
            } else if is_dst_ending {
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

    fn display_opening_ending_info(&self, result: SearchResult) {
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
        src_idx: usize,
        dst_idx: usize,
        frame_hash_map: &[FrameHashes],
    ) -> Result<OpeningAndEndingInfo> {
        tracing::debug!("started audio comparator");

        let (src_frame_hashes, dst_frame_hashes) =
            (&frame_hash_map[src_idx], &frame_hash_map[dst_idx]);

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
        if matches.len() == 0 {
            return None;
        }

        let mut candidates = Vec::new();

        for (m, is_source) in matches {
            if *is_source {
                for e in &m.src_openings {
                    let o = (e.src_longest_run, e.src_hash_duration, e.src_match_hash);
                    candidates.push((o, true));
                }
                for e in &m.src_endings {
                    let o = (e.src_longest_run, e.src_hash_duration, e.src_match_hash);
                    candidates.push((o, false));
                }
            } else {
                for e in &m.dst_openings {
                    let o = (e.dst_longest_run, e.dst_hash_duration, e.dst_match_hash);
                    candidates.push((o, true));
                }
                for e in &m.dst_endings {
                    let o = (e.dst_longest_run, e.dst_hash_duration, e.dst_match_hash);
                    candidates.push((o, false));
                }
            }
        }

        let mut distinct_matches: HashMap<usize, HashSet<usize>> = HashMap::new();

        for (i, (c, _)) in candidates.iter().enumerate() {
            for (j, (other, _)) in candidates.iter().enumerate() {
                let dist = u32::count_ones(c.2 ^ other.2);

                // Add a small bias to the hash match threshold when comparing sequence hashes.
                if dist >= self.hash_match_threshold + (self.hash_match_threshold / 2) {
                    continue;
                }

                distinct_matches
                    .entry(i)
                    .or_insert(HashSet::new())
                    .insert(j);
                distinct_matches
                    .entry(j)
                    .or_insert(HashSet::new())
                    .insert(i);
            }
        }

        let mut best_openings = distinct_matches
            .iter()
            .filter(|(k, _)| {
                let (_, is_opening) = candidates[**k];
                is_opening
            })
            .map(|(k, v)| {
                let (((start, end), _, _), _) = candidates[*k];
                let count = v.len() as i64;
                let duration_secs = (end - start).as_secs_f32();
                // Weighted sum of count and duration, with more weight given to duration.
                (-(count as f32 * 0.3 + duration_secs * 0.7), *k)
            })
            .collect::<Vec<_>>();
        // We can't used .sort() because f32 is not `Ord`.
        best_openings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mut best_endings = distinct_matches
            .iter()
            .filter(|(k, _)| {
                let (_, is_opening) = candidates[**k];
                !is_opening
            })
            .map(|(k, v)| {
                let (((start, end), _, _), _) = candidates[*k];
                let count = v.len() as i64;
                let duration_secs = (end - start).as_secs_f32();
                // Weighted sum of count and duration, with more weight given to duration.
                (-(count as f32 * 0.3 + duration_secs * 0.7), *k)
            })
            .collect::<Vec<_>>();
        best_endings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mut best: SearchResult = Default::default();

        if let Some((_, idx)) = best_openings.first() {
            let (((start, end), hash_duration, _), _) = candidates[*idx];
            best.opening = Some((
                // Add a buffer between actual detected times and what we return to users.
                start + self.time_padding,
                // Adjust ending time using the configured hash duration.
                end - self.time_padding - hash_duration,
            ));
        }
        if let Some((_, idx)) = best_endings.first() {
            let (((start, end), hash_duration, _), _) = candidates[*idx];
            best.ending = Some((
                // Add a buffer between actual detected times and what we return to users.
                start + self.time_padding,
                // Adjust ending time using the configured hash duration.
                end - self.time_padding - hash_duration,
            ));
        }

        Some(best)
    }
}

impl<P: AsRef<Path> + Ord + Sync> Comparator<P> {
    /// Runs the comparator.
    ///
    /// * If `analyze` is set to true, an `Analyzer` will be built for each video file and run in-place.
    /// * If `use_skip_files` is set, if a skip file already exists for a video, the video will be skipped during this run. If `write_skip_files`
    /// is set, a skip file will be written to disk once the comparator is completed.
    /// * If `display` is set, the final results will be printed to stdout.
    pub fn run(
        &self,
        analyze: bool,
        display: bool,
        use_skip_files: bool,
        write_skip_files: bool,
        threading: bool,
    ) -> Result<BTreeMap<PathBuf, SearchResult>> {
        // Build a list of video pairs for actual search. Pairs should only appear once.
        // Given N videos, this will result in: (N * (N-1)) / 2 pairs
        let mut pairs = Vec::new();
        let mut processed_videos = vec![false; self.videos.len()];

        // Stores frame hash data for each video to be analyzed.
        // We load them all now to be able to handle in-place analysis when the `analyze`
        // flag is passed in to this method.
        let mut frame_hash_map = Vec::new();

        for (i, v1) in self.videos.iter().enumerate() {
            let v1 = v1.as_ref();

            let frame_hashes = FrameHashes::from_video(v1, analyze)?;

            frame_hash_map.push(frame_hashes);

            for j in 0..self.videos.len() {
                if i == j || processed_videos[j] {
                    continue;
                }
                pairs.push((i, j));
            }
            processed_videos[i] = true;
        }

        let mut data = Vec::new();

        if cfg!(feature = "rayon") && threading {
            // Perform the search in parallel for all pairs.
            #[cfg(feature = "rayon")]
            {
                data = pairs
                    .par_iter()
                    .map(|(src_idx, dst_idx)| {
                        (
                            *src_idx,
                            *dst_idx,
                            self.search(*src_idx, *dst_idx, &frame_hash_map).unwrap(),
                        )
                    })
                    .filter(|(_, _, info)| !info.is_empty())
                    .collect::<Vec<_>>();
            }
        } else {
            data.extend(
                pairs
                    .iter()
                    .map(|(src_idx, dst_idx)| {
                        (
                            *src_idx,
                            *dst_idx,
                            self.search(*src_idx, *dst_idx, &frame_hash_map).unwrap(),
                        )
                    })
                    .filter(|(_, _, info)| !info.is_empty()),
            );
        }

        // This map tracks the generated info struct for each video path. A bool is included
        // to allow determining whether the path is a source (true) or dest (false) in the info
        // struct.
        let mut info_map: Vec<Vec<(&OpeningAndEndingInfo, bool)>> =
            vec![Vec::new(); self.videos.len()];
        for (src_idx, dst_idx, info) in &data {
            info_map[*src_idx].push((info, true));
            info_map[*dst_idx].push((info, false));
        }

        let mut match_map = BTreeMap::new();

        // For each path, find the best opening and ending candidate among the list
        // of other videos. If required, display the result and write a skip file to disk.
        for (idx, matches) in info_map.into_iter().enumerate() {
            let path = self.videos[idx].as_ref().to_owned();
            if display {
                println!("\n{}\n", path.display());
            }

            // Skip match selection for this video if it already has a skip file on disk.
            if use_skip_files && Self::check_skip_file(&path)? {
                if display {
                    println!("Skipping due to existing skip file...");
                }
                continue;
            }

            let result = self.find_best_match(&matches);
            if result.is_none() {
                if display {
                    println!("No opening or ending found.");
                }
                continue;
            }
            let result = result.unwrap();
            if display {
                self.display_opening_ending_info(result);
            }
            if write_skip_files {
                self.create_skip_file(&path, result)?;
            }
            match_map.insert(path, result.clone());
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
        // TODO(aksiksi): Make this test actually do something. Right now, it doesn't really detect anything
        // because the clips are too short.
        let paths = get_sample_paths();
        let comparator = Comparator::from_files(paths)
            .with_min_opening_duration(Duration::from_millis(300))
            .with_min_ending_duration(Duration::from_millis(300));
        let data = comparator.run(true, true, false, false, false).unwrap();
        assert_eq!(data.len(), 2);
    }
}
