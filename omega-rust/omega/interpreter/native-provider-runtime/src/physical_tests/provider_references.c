#if !defined(_WIN32)
#define _GNU_SOURCE
#endif
#include <errno.h>
#include <stdint.h>
#if defined(_WIN32)
#include <windows.h>
#define EXPORT __declspec(dllexport)
#else
#include <sys/mman.h>
#define EXPORT

/* Loading another provider image must not replace the guest's TLS error. */
__attribute__((constructor)) static void poison_loader_errno(void) {
    errno = E2BIG;
}
#endif

static struct {
    uint32_t prefix;
    _Alignas(8) int32_t cell;
    uint32_t suffix;
} storage = {0x13579bdf, 37, 0x2468ace0};

EXPORT const int32_t *provider_reference_cell(void) { return &storage.cell; }

/* This tests physical cell identity only. It does not assert an Omega shared
 * reference stability contract permitting the foreign mutation. */
EXPORT uint64_t provider_reference_change(void) {
    storage.cell = 91;
    return 91;
}

EXPORT uint64_t provider_reference_guards(void) {
    return ((uint64_t)storage.prefix << 32) | storage.suffix;
}

EXPORT const int32_t *provider_reference_null(void) { return 0; }

EXPORT const int32_t *provider_reference_misaligned(void) {
    return (const int32_t *)((const unsigned char *)&storage.cell + 1);
}

/* A faulty provider can return a nonnull aligned address with inaccessible
 * backing. Reading it must fault its worker, never fabricate a scalar. */
EXPORT const int32_t *provider_reference_inaccessible(void) {
#if defined(_WIN32)
    return VirtualAlloc(0, 4096, MEM_COMMIT | MEM_RESERVE, PAGE_NOACCESS);
#else
    void *page = mmap(0, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    return page == MAP_FAILED ? 0 : page;
#endif
}

EXPORT uint64_t provider_reference_success_changes_errno(void) {
    errno = ENOENT;
    return 37;
}
