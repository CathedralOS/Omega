#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#if defined(_WIN32)
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif

static const uint64_t *original;

static void observe_at_exit(void) {
    const char *path = getenv("OMEGA_NATIVE_EXIT_WITNESS");
    if (path == NULL || original == NULL) {
        abort();
    }
    FILE *output = fopen(path, "wb");
    if (output == NULL) {
        abort();
    }
    uint64_t value = *original;
    if (fwrite(&value, sizeof(value), 1, output) != 1 || fclose(output) != 0) {
        abort();
    }
}

EXPORT void retention_exit_register(const uint64_t *home) {
    if (original != NULL || home == NULL || atexit(observe_at_exit) != 0) {
        abort();
    }
    original = home;
}
