#include <stdio.h>
#include <needle.h>

int main() {
    NeedleError err;
    const NeedleAudioAnalyzer *analyzer = NULL;

    const char *video_paths[] = {
        "/Users/aksiksi/Movies/ep1.mkv",
        "/Users/aksiksi/Movies/ep2.mkv",
    };
    const int NUM_PATHS = 2;

    // Setup the analyzer and comparator.
    err = needle_audio_analyzer_new(video_paths, NUM_PATHS, false, false, &analyzer);
    if (err != 0) {
        printf("Failed to create analyzer: %s\n", needle_error_to_str(err));
        goto done;
    }

    needle_audio_analyzer_print_paths(analyzer);

    err = needle_audio_analyzer_run(analyzer, 0.3, 3.0, false);
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
