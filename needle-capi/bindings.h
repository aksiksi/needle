#ifndef NEEDLE_H
#define NEEDLE_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum NeedleError {
  NeedleError_Ok = 0,
  NeedleError_InvalidUtf8String,
  NeedleError_NullArgument,
  NeedleError_FrameHashDataNotFound,
  NeedleError_InvalidFrameHashData,
  NeedleError_ComparatorMinimumPaths,
  NeedleError_AnalyzerInvalidHashPeriod,
  NeedleError_AnalyzerInvalidHashDuration,
  NeedleError_IOError,
  NeedleError_Unknown,
} NeedleError;

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
 * NeedleAudioAnalyzer *analyzer = NULL;
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
 * NeedleAudioComparator *comparator = NULL;
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
 *                                   20.0,
 *                                   10.0,
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
 * Constructs a new [NeedleAudioAnalyzer].
 */
enum NeedleError needle_audio_analyzer_new(const char *const *paths,
                                           size_t num_paths,
                                           bool threaded_decoding,
                                           bool force,
                                           const struct NeedleAudioAnalyzer **output);

void needle_audio_analyzer_free(const struct NeedleAudioAnalyzer *analyzer);

enum NeedleError needle_audio_analyzer_run(struct NeedleAudioAnalyzer *analyzer,
                                           float hash_period,
                                           float hash_duration,
                                           bool persist);

/**
 * Constructs a new [NeedleAudioComparator].
 */
enum NeedleError needle_audio_comparator_new(const char *const *paths,
                                             size_t num_paths,
                                             uint16_t hash_match_threshold,
                                             float opening_search_percentage,
                                             float ending_search_percentage,
                                             float min_opening_duration,
                                             float min_ending_duration,
                                             float time_padding,
                                             const struct NeedleAudioComparator **output);

void needle_audio_comparator_free(const struct NeedleAudioComparator *comparator);

enum NeedleError needle_audio_comparator_run(struct NeedleAudioComparator *comparator,
                                             bool analyze,
                                             bool display,
                                             bool use_skip_files);

#endif /* NEEDLE_H */
