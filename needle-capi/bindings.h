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

typedef struct Analyzer Analyzer;

typedef struct Comparator Comparator;

const char *needle_error_to_str(enum NeedleError error);

enum NeedleError needle_audio_analyzer_new(const char *const *paths,
                                           size_t num_paths,
                                           bool threaded_decoding,
                                           bool force,
                                           const struct Analyzer **output);

void needle_audio_analyzer_free(const struct Analyzer *analyzer);

enum NeedleError needle_audio_analyzer_run(struct Analyzer *_analyzer,
                                           float _hash_period,
                                           float _hash_duration,
                                           bool _persist);

enum NeedleError needle_audio_comparator_new(const char *const *paths,
                                             size_t num_paths,
                                             uint16_t hash_match_threshold,
                                             float opening_search_percentage,
                                             float ending_search_percentage,
                                             float min_opening_duration,
                                             float min_ending_duration,
                                             float time_padding,
                                             const struct Comparator **output);

void needle_audio_comparator_free(const struct Comparator *comparator);

enum NeedleError needle_audio_comparator_run(struct Comparator *_comparator,
                                             bool _analyze,
                                             bool _display,
                                             bool _use_skip_files);

#endif /* NEEDLE_H */
