#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <unistd.h>
#include <signal.h>
#include <sys/wait.h>

struct ByteView { uint8_t *bytes; uint64_t length; };
struct LineReadResult { uint32_t tag; uint32_t padding; uint64_t count; };
extern struct LineReadResult omega_entry(const struct ByteView *out);
_Static_assert(sizeof(struct ByteView) == 16, "descriptor extent");
_Static_assert(offsetof(struct LineReadResult, count) == 8, "result payload offset");
_Static_assert(sizeof(struct LineReadResult) == 16, "result extent");

static int check_reads(const uint8_t *input, size_t length,
                       const uint8_t *capacities, size_t calls) {
    int descriptors[2];
    if (pipe(descriptors)) return 1;
    if (write(descriptors[1], input, length) != (ssize_t)length) return 2;
    close(descriptors[1]);
    if (dup2(descriptors[0], 0) < 0) return 3;
    if (descriptors[0] != 0) close(descriptors[0]);
    size_t consumed = 0;
    for (size_t invocation = 0; invocation < calls; ++invocation) {
        uint8_t storage[18];
        memset(storage, 0xa5, sizeof(storage));
        size_t capacity = capacities[invocation];
        if (capacity > sizeof(storage) - 2) return 4;
        struct ByteView view = {capacity ? storage + 1 : NULL, capacity};
        uint32_t expected_tag = 3;
        size_t expected_count = 0;
        while (expected_count < capacity) {
            if (consumed + expected_count == length) { expected_tag = 2; break; }
            uint8_t byte = input[consumed + expected_count++];
            if (byte == 10) { expected_tag = 1; break; }
        }
        struct LineReadResult result = omega_entry(&view);
        if (result.tag != expected_tag || result.count != expected_count) return 5;
        if (result.padding != 0) return 6;
        if (view.bytes != (capacity ? storage + 1 : NULL) || view.length != capacity) return 7;
        for (size_t position = 0; position < sizeof(storage); ++position) {
            uint8_t expected = position > 0 && position <= expected_count
                ? input[consumed + position - 1] : 0xa5;
            if (storage[position] != expected) return 8;
        }
        consumed += expected_count;
    }
    // A full or zero-capacity call must leave the next byte unread, even at EOF/LF.
    uint8_t remaining[32];
    ssize_t remaining_count = read(0, remaining, sizeof(remaining));
    if (remaining_count != (ssize_t)(length - consumed)) return 9;
    if (memcmp(remaining, input + consumed, (size_t)remaining_count)) return 10;
    return 0;
}

int main(void) {
    alarm(20);
    const uint8_t capacities[] = {0, 1, 2, 0, 3, 8, 16, 1};
    for (unsigned octet = 0; octet <= 255; ++octet) {
        const uint8_t input[] = {(uint8_t)octet, 13, 10, 0, 0xc3, 0xa9, 10, 0xff};
        for (uint8_t capacity = 0; capacity <= sizeof(input) + 1; ++capacity) {
            for (size_t length = 0; length <= sizeof(input); ++length) {
                int result = check_reads(input, length, &capacity, 1);
                if (result) return result;
            }
        }
        int result = check_reads(input, sizeof(input), capacities, sizeof(capacities));
        if (result) return result;
    }
    // Even unavailable input is untouched for an empty view; a nonempty read
    // must trap rather than fabricate Invalid, EOF, or a normal count.
    pid_t child = fork();
    if (child < 0) return 11;
    if (child == 0) {
        alarm(5);
        close(0);
        struct ByteView empty = {NULL, 0};
        struct LineReadResult result = omega_entry(&empty);
        if (result.tag != 3 || result.count != 0) _exit(12);
        uint8_t byte = 0xa5;
        struct ByteView view = {&byte, 1};
        omega_entry(&view);
        _exit(13);
    }
    int status;
    if (waitpid(child, &status, 0) != child || !WIFSIGNALED(status)) return 14;
    if (WTERMSIG(status) != SIGILL && WTERMSIG(status) != SIGTRAP) return 15;
    return 0;
}
