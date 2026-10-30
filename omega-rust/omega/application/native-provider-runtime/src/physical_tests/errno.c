#include <errno.h>
#include <stdint.h>
#include <string.h>

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

/* The one semantic ByteView is realized as these pointer/length ABI lanes. */
uint64_t errno_copy_view(uint8_t *destination, uint64_t length) {
    if (length != sizeof(int)) return 0;
    memcpy(destination, &errno, sizeof(int));
    return length;
}
