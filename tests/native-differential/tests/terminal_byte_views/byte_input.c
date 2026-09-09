#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>

struct ByteView { uint8_t *bytes; uint64_t length; };
_Static_assert(sizeof(struct ByteView) == 16, "descriptor extent");
_Static_assert(offsetof(struct ByteView, length) == 8, "descriptor length");
/* Scalar inputs precede structural descriptor pointers in this native ABI. */
extern void omega_entry(int32_t expected, const struct ByteView *out);

static int supply(const uint8_t *bytes, size_t length) {
    int descriptors[2];
    if (pipe(descriptors) != 0) return 1;
    if (length && write(descriptors[1], bytes, length) != (ssize_t)length) return 2;
    close(descriptors[1]);
    if (dup2(descriptors[0], 0) < 0) return 3;
    if (descriptors[0] != 0) close(descriptors[0]);
    return 0;
}

int main(void) {
    alarm(20);
    for (unsigned octet = 0; octet <= 255; ++octet) {
        uint8_t input[] = {(uint8_t)octet, (uint8_t)(octet ^ 0xff), 10, 0x80, 0x42};
        uint8_t expected[] = {1, input[1] == octet, input[2] == octet};
        if (supply(input, sizeof(input))) return 1;
        struct { uint8_t before[8], bytes[3], after[8]; } storage;
        memset(&storage, 0xa5, sizeof(storage));
        struct ByteView empty = {NULL, 0};
        omega_entry((int32_t)octet, &empty);
        if (empty.bytes || empty.length) return 2;
        struct ByteView view = {storage.bytes, 3};
        omega_entry((int32_t)octet, &view);
        if (memcmp(storage.bytes, expected, 3) != 0) return 3;
        if (view.bytes != storage.bytes || view.length != 3) return 4;
        view.length = 1;
        omega_entry((int32_t)octet, &view);
        if (storage.bytes[0] != (input[3] == octet)) return 4;
        if (view.bytes != storage.bytes || view.length != 1) return 5;
        if (storage.bytes[1] != expected[1] || storage.bytes[2] != expected[2]) return 6;
        for (size_t position = 0; position < 8; ++position) {
            if (storage.before[position] != 0xa5 || storage.after[position] != 0xa5) return 7;
        }
        uint8_t following;
        if (read(0, &following, 1) != 1 || following != 0x42) return 8;
        omega_entry((int32_t)octet, &view);
        if (storage.bytes[0] != (input[3] == octet)) return 9;
        if (supply(input, 1)) return 13;
        view.length = 3;
        omega_entry((int32_t)octet, &view);
        if (storage.bytes[0] != expected[0] || storage.bytes[1] != expected[1] || storage.bytes[2] != expected[2]) return 14;
    }
    /* Failed input must trap, not fabricate EOF or perform a write. */
    pid_t child = fork();
    if (child < 0) return 10;
    if (child == 0) {
        close(0);
        uint8_t byte = 0xa5;
        struct ByteView view = {&byte, 1};
        omega_entry(0, &view);
        _exit(11);
    }
    int status;
    if (waitpid(child, &status, 0) != child || !WIFSIGNALED(status)) return 12;
    return 0;
}
