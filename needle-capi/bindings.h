#ifndef NEEDLE_H
#define NEEDLE_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum NeedleError {
  NeedleError_None = 0,
  NeedleError_InvalidUtf8String,
  NeedleError_NullArgument,
  NeedleError_ComparatorMinimumPaths,
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
 * NeedleAnalyzer *analyzer = NULL;
 *
 * err = needle_audio_analyzer_new(..., &analyzer);
 * if (err != 0) {
 *     printf("Failed to create analyzer: %s\n", needle_error_to_str(err));
 *     return;
 * }
 *
 * err = needle_audio_analyzer_run(analyzer, ...);
 * if (err != 0) {
 *     printf("Failed to run analyzer: %s\n", needle_error_to_str(err));
 *     return;
 * }
 *
 * needle_audio_analyzer_free(analyzer);
 * ```
 */
typedef struct NeedleAnalyzer NeedleAnalyzer;

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
 * NeedleComparator *comparator = NULL;
 *
 * err = needle_audio_comparator_new(..., &comparator);
 * if (err != 0) {
 *     printf("Failed to create comparator: %s\n", needle_error_to_str(err));
 *     return;
 * }
 *
 * err = needle_audio_comparator_run(comparator, ...);
 * if (err != 0) {
 *     printf("Failed to run comparator: %s\n", needle_error_to_str(err));
 *     return;
 * }
 *
 * needle_audio_comparator_free(comparator);
 * ```
 */
typedef struct NeedleComparator NeedleComparator;

/**
 * Returns the string representation of the given [NeedleError].
 */
const char *needle_error_to_str(enum NeedleError error);

/**
 * Constructs a new [NeedleAnalyzer].
 */
enum NeedleError needle_audio_analyzer_new(const char *const *paths,
                                           size_t num_paths,
                                           bool threaded_decoding,
                                           bool force,
                                           const struct NeedleAnalyzer **output);

void needle_audio_analyzer_free(const struct NeedleAnalyzer *analyzer);

enum NeedleError needle_audio_analyzer_run(struct NeedleAnalyzer *_analyzer,
                                           float _hash_period,
                                           float _hash_duration,
                                           bool _persist);

/**
 * Constructs a new [NeedleComparator].
 */
enum NeedleError needle_audio_comparator_new(const char *const *paths,
                                             size_t num_paths,
                                             uint16_t hash_match_threshold,
                                             float opening_search_percentage,
                                             float ending_search_percentage,
                                             float min_opening_duration,
                                             float min_ending_duration,
                                             float time_padding,
                                             const struct NeedleComparator **output);

void needle_audio_comparator_free(const struct NeedleComparator *comparator);

enum NeedleError needle_audio_comparator_run(struct NeedleComparator *_comparator,
                                             bool _analyze,
                                             bool _display,
                                             bool _use_skip_files);

#endif /* NEEDLE_H */
