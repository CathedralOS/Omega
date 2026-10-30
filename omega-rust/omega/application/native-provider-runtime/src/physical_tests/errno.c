#include <errno.h>
#include <stdint.h>

/* Image loading is worker housekeeping, not a guest call. Two distinct images
 * built from this source force it to overwrite the actual worker TLS slot. */
__attribute__((constructor)) static void poison_loader_errno(void) {
    errno = E2BIG;
}

/* A successful guest call may change errno; retain the actual value rather
 * than deriving it from the call's success/failure result. */
int64_t errno_success_changes(void) {
    errno = ENOENT;
    return 37;
}
