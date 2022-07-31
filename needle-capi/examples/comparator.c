#include <stdio.h>
#include <needle.h>

int main() {
    NeedleError err;
    const NeedleAudioComparator *comparator = NULL;

    const char *video_paths[] = {
        "../../needle/resources/sample-5s.mp4",
        "../../needle/resources/sample-shifted-4s.mp4",
    };
    const int NUM_PATHS = 2;

    err = needle_audio_comparator_new_default(video_paths, NUM_PATHS, &comparator);
    if (err != 0) {
        printf("Failed to create comparator: %s\n", needle_error_to_str(err));
        goto done;
    }

    err = needle_audio_comparator_run(comparator, true, true, false, false, true);
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
