#include <stdio.h>
#include <needle.h>

int main() {
    NeedleError err;
    NeedleAudioAnalyzer *analyzer = NULL;
    const NeedleAudioComparator *comparator = NULL;
    const char *const *video_paths = NULL;
    size_t num_video_paths = 0;

    const char *paths[] = {
        "../../needle/resources/sample-5s.mp4",
        "../../needle/resources/sample-shifted-4s.mp4",
    };
    const int NUM_PATHS = 2;

    // Find valid video paths.
    err = needle_util_find_video_files(paths, NUM_PATHS, true, true, &video_paths, &num_video_paths);
    if (err != 0) {
        printf("Failed to find valid videos: %s\n", needle_error_to_str(err));
        goto done;
    }

    // Setup the analyzer and comparator.
    err = needle_audio_analyzer_new_default(video_paths, num_video_paths, &analyzer);
    if (err != 0) {
        printf("Failed to create analyzer: %s\n", needle_error_to_str(err));
        goto done;
    }
    err = needle_audio_comparator_new_default(video_paths, num_video_paths, &comparator);
    if (err != 0) {
        printf("Failed to create comparator: %s\n", needle_error_to_str(err));
        goto done;
    }

    // Print analyzer paths.
    needle_audio_analyzer_print_paths(analyzer);

    err = needle_audio_analyzer_run(analyzer, 0.3, 3.0, false, true);
    if (err != 0) {
        printf("Failed to run analyzer: %s\n", needle_error_to_str(err));
        goto done;
    }

    done:
    if (analyzer != NULL) {
        needle_audio_analyzer_free(analyzer);
    }
    if (comparator != NULL) {
        needle_audio_comparator_free(comparator);
    }
    if (video_paths != NULL) {
        needle_util_video_files_free(video_paths, num_video_paths);
    }

    return 0;
}
