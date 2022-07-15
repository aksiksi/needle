#include <stdio.h>
#include <needle.h>

int main() {
    NeedleError err;
    const NeedleAudioComparator *comparator = NULL;

    const char *video_paths[] = {
        "/Users/aksiksi/Movies/ep1.mkv",
        "/Users/aksiksi/Movies/ep2.mkv",
    };
    const int NUM_PATHS = 2;

    err = needle_audio_comparator_new(video_paths, NUM_PATHS,
                                      10,
                                      0.33,
                                      0.25,
                                      20.0,
                                      10.0,
                                      0.0,
                                      &comparator);
    if (err != 0) {
        printf("Failed to create comparator: %s\n", needle_error_to_str(err));
        goto done;
    }

    err = needle_audio_comparator_run(comparator, true, true, false);
    if (err != 0) {
        printf("Failed to run comparator: %s\n", needle_error_to_str(err));
        goto done;
    }

    done:
    if (comparator != NULL) {
        needle_audio_comparator_free(comparator);
    }

    return 0;
}
