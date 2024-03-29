#include <stdio.h>
#include <needle.h>

int main() {
    NeedleError err;
    NeedleAudioAnalyzer *analyzer = NULL;

    const char *video_paths[] = {
        "../../needle/resources/sample-5s.mp4",
        "../../needle/resources/sample-shifted-4s.mp4",
    };
    const int NUM_PATHS = 2;

    // Setup the analyzer and comparator.
    err = needle_audio_analyzer_new_default(video_paths, NUM_PATHS, &analyzer);
    if (err != 0) {
        printf("Failed to create analyzer: %s\n", needle_error_to_str(err));
        goto done;
    }

    needle_audio_analyzer_print_paths(analyzer);

    err = needle_audio_analyzer_run(analyzer, 0.3, false, true);
    if (err != 0) {
        printf("Failed to run analyzer: %s\n", needle_error_to_str(err));
        goto done;
    }

    done:
    if (analyzer != NULL) {
        needle_audio_analyzer_free(analyzer);
    }

    return 0;
}
