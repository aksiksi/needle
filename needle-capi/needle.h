#ifndef NEEDLE_H
#define NEEDLE_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * C error enum that extends [needle::Error].
 */
typedef enum NeedleError {
  /**
   * No error.
   */
  NeedleError_Ok = 0,
  /**
   * Invalid UTF-8 string.
   */
  NeedleError_InvalidUtf8String,
  /**
   * One or more pointer arguments passed into the function were NULL.
   */
  NeedleError_NullArgument,
  /**
   * One or more arguments were invalid (usually zero).
   */
  NeedleError_InvalidArgument,
  /**
   * Frame hash data was not found on disk.
   */
  NeedleError_FrameHashDataNotFound,
  /**
   * Frame hash data on disk has an invalid version.
   */
  NeedleError_FrameHashDataInvalidVersion,
  /**
   * Frame hash data on disk is not valid.
   */
  NeedleError_InvalidFrameHashData,
  /**
   * Comparator needs at least two video paths.
   */
  NeedleError_ComparatorMinimumPaths,
  /**
   * Analyzer hash period specified was invalid.
   */
  NeedleError_AnalyzerInvalidHashPeriod,
  /**
   * Analyzer hash duration specified was too short.
   */
  NeedleError_AnalyzerInvalidHashDuration,
  /**
   * Wraps a [std::io::Error].
   */
  NeedleError_IOError,
  /**
   * Unknown error.
   */
  NeedleError_Unknown,
} NeedleError;

/**
 * TODO
 */
typedef struct FrameHashes FrameHashes;

/**
 * Wraps [needle::audio::Analyzer] with a C API.
 *
 * # Example
 *
 * ```c
 * #include <stdio.h>
 * #include <needle.h>
 *
 * NeedleError err;
 * const NeedleAudioAnalyzer *analyzer = NULL;
 *
 * char *video_paths[] = {
 *     "/tmp/abcd.mkv",
 *     "/tmp/efgh.mp4",
 * };
 * const int NUM_PATHS = 2;
 *
 * err = needle_audio_analyzer_new(paths, NUM_PATHS, false, false, &analyzer);
 * if (err != 0) {
 *     printf("Failed to create analyzer: %s\n", needle_error_to_str(err));
 *     return;
 * }
 *
 * err = needle_audio_analyzer_run(analyzer, 0.3, 3.0, true);
 * if (err != 0) {
 *     printf("Failed to run analyzer: %s\n", needle_error_to_str(err));
 * }
 *
 * needle_audio_analyzer_free(analyzer);
 * ```
 */
typedef struct NeedleAudioAnalyzer NeedleAudioAnalyzer;

/**
 * Wraps [needle::audio::Comparator] with a C API.
 *
 * # Example
 *
 * ```c
 * #include <stdio.h>
 * #include <needle.h>
 *
 * NeedleError err;
 * const NeedleAudioComparator *comparator = NULL;
 *
 * char *video_paths[] = {
 *     "/tmp/abcd.mkv",
 *     "/tmp/efgh.mp4",
 * };
 * const int NUM_PATHS = 2;
 *
 * err = needle_audio_comparator_new(paths, NUM_PATHS,
 *                                   10,
 *                                   0.33,
 *                                   0.25,
 *                                   20,
 *                                   10,
 *                                   0.0,
 *                                   &comparator);
 * if (err != 0) {
 *     printf("Failed to create comparator: %s\n", needle_error_to_str(err));
 *     return;
 * }
 *
 * err = needle_audio_comparator_run(comparator, ...);
 * if (err != 0) {
 *     printf("Failed to run comparator: %s\n", needle_error_to_str(err));
 * }
 *
 * needle_audio_comparator_free(comparator);
 * ```
 */
typedef struct NeedleAudioComparator NeedleAudioComparator;

/**
 * Returns the string representation of the given [NeedleError].
 */
const char *needle_error_to_str(enum NeedleError error);

/**
 * Given a list of paths (files or directories), returns the list of valid video files.
 *
 * When you are done with the returned list of videos, you must call [needle_util_video_files_free]
 * to free the memory.
 *
 * For more information, refer to [needle::util::find_video_files].
 */
enum NeedleError needle_util_find_video_files(const char *const *paths,
                                              size_t num_paths,
                                              bool full,
                                              bool audio,
                                              const char *const **videos,
                                              size_t *num_videos);

/**
 * Free the list of videos files returned by [needle_util_find_video_files].
 */
void needle_util_video_files_free(const char *const *videos, size_t num_videos);

/**
 * Constructs a new [NeedleAudioAnalyzer] with sane defaults.
 */
enum NeedleError needle_audio_analyzer_new_default(const char *const *paths,
                                                   size_t num_paths,
                                                   struct NeedleAudioAnalyzer **output);

/**
 * Constructs a new [NeedleAudioAnalyzer].
 */
enum NeedleError needle_audio_analyzer_new(const char *const *paths,
                                           size_t num_paths,
                                           float opening_search_percentage,
                                           float ending_search_percentage,
                                           bool include_endings,
                                           bool threaded_decoding,
                                           bool force,
                                           struct NeedleAudioAnalyzer **output);

/**
 * Returns the [FrameHashes] for the video at the given index.
 *
 * Note that this index must match the index of the video provided to the [Analyzer].
 */
enum NeedleError needle_audio_analyzer_get_frame_hashes(const struct NeedleAudioAnalyzer *analyzer,
                                                        size_t index,
                                                        const struct FrameHashes **output);

/**
 * Free the provided [NeedleAudioAnalyzer].
 */
void needle_audio_analyzer_free(const struct NeedleAudioAnalyzer *analyzer);

/**
 * Print the video paths tracked by this [NeedleAudioAnalyzer].
 */
void needle_audio_analyzer_print_paths(const struct NeedleAudioAnalyzer *analyzer);

/**
 * Run the [NeedleAudioAnalyzer].
 */
enum NeedleError needle_audio_analyzer_run(struct NeedleAudioAnalyzer *analyzer,
                                           bool persist,
                                           bool threading);

/**
 * Constructs a new [NeedleAudioComparator] using sane defaults.
 *
 * Refer to the library to see these values.
 */
enum NeedleError needle_audio_comparator_new_default(const char *const *paths,
                                                     size_t num_paths,
                                                     const struct NeedleAudioComparator **output);

/**
 * Constructs a new [NeedleAudioComparator].
 */
enum NeedleError needle_audio_comparator_new(const char *const *paths,
                                             size_t num_paths,
                                             bool include_endings,
                                             uint16_t hash_match_threshold,
                                             uint16_t min_opening_duration,
                                             uint16_t min_ending_duration,
                                             float time_padding,
                                             const struct NeedleAudioComparator **output);

/**
 * Free the provided [NeedleAudioComparator].
 */
void needle_audio_comparator_free(const struct NeedleAudioComparator *comparator);

/**
 * Run the [NeedleAudioComparator].
 */
enum NeedleError needle_audio_comparator_run(const struct NeedleAudioComparator *comparator,
                                             bool analyze,
                                             bool display,
                                             bool use_skip_files,
                                             bool write_skip_files,
                                             bool threading);

#endif /* NEEDLE_H */
