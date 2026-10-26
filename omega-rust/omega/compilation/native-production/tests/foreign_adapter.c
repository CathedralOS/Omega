#if defined(_MSC_VER) && !defined(_CRT_SECURE_NO_WARNINGS)
// These fixed test inputs use the standard C environment/file interfaces.
#define _CRT_SECURE_NO_WARNINGS
#endif
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#include <sys/resource.h>
#endif
struct frame { const void *const *arguments; void *result; };
typedef void (*adapter)(struct frame *);
int main(int argc, char **argv) {
    assert(argc == 9);
#ifdef _WIN32
    SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
#else
    const struct rlimit core_limit = {0, 0};
    assert(setrlimit(RLIMIT_CORE, &core_limit) == 0);
#endif
    adapter calls[4];
    for (int n = 0; n < 4; n++) {
#ifdef _WIN32
        HMODULE library = LoadLibraryExA(argv[1+2*n], NULL, LOAD_WITH_ALTERED_SEARCH_PATH);
        if (!library) { fprintf(stderr, "LoadLibrary: %lu\n", GetLastError()); return 1; }
        FARPROC symbol = GetProcAddress(library, argv[2+2*n]);
#else
        void *library = dlopen(argv[1+2*n], RTLD_NOW | RTLD_LOCAL);
        if (!library) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 1; }
        void *symbol = dlsym(library, argv[2+2*n]);
        if (!symbol) { fprintf(stderr, "dlsym: %s\n", dlerror()); return 1; }
#endif
        assert(sizeof calls[n] == sizeof symbol);
        memcpy(&calls[n], &symbol, sizeof symbol);
    }
    uint64_t capacity = 0, pending = 0, returned = 0, root = 37, observed = 0;
    struct frame frame = {NULL, &capacity};
    calls[0](&frame);
    const uint64_t *source = &root;
    const void *arguments[] = {&capacity, &source};
    frame = (struct frame){arguments, &pending};
    calls[1](&frame);
    assert(capacity != 0 && pending != 0);
    uint64_t *destination = &observed;
    arguments[0] = &pending; arguments[1] = &destination;
    frame = (struct frame){arguments, &returned};
    calls[2](&frame);
    assert(returned == capacity && observed == 37 && root == 37);
    if (getenv("OMEGA_FOREIGN_REPEAT_COMPLETION")) {
        // The real provider must trap on a stale registration. This child is
        // also the fault containment boundary for this physical transport test.
        calls[2](&frame);
        return 99;
    }
    arguments[0] = &returned;
    frame = (struct frame){arguments, NULL};
    calls[3](&frame);
    return 0;
}
