#include <stdint.h>
#include <stddef.h>
#include <unistd.h>

struct ByteView { uint8_t *bytes; uint64_t length; };
extern void omega_entry(const struct ByteView *out);

int main(void) {
    alarm(20);
    for (unsigned octet = 0; octet <= 255; ++octet) {
        int descriptors[2];
        if (pipe(descriptors)) return 1;
        uint8_t input[2] = {(uint8_t)octet, (uint8_t)(octet ^ 0xff)};
        if (write(descriptors[1], input, 2) != 2) return 2;
        close(descriptors[1]);
        if (dup2(descriptors[0], 0) < 0) return 3;
        if (descriptors[0] != 0) close(descriptors[0]);
        struct ByteView empty = {NULL, 0};
        omega_entry(&empty);
        if (empty.bytes || empty.length) return 4;
        uint8_t storage[3] = {0xa5, 0xa5, 0xa5};
        struct ByteView view = {storage + 1, 1};
        omega_entry(&view);
        if (storage[1] != input[0] || storage[0] != 0xa5 || storage[2] != 0xa5) return 5;
        if (view.bytes != storage + 1 || view.length != 1) return 6;
        omega_entry(&view);
        if (storage[1] != input[1]) return 7;
        omega_entry(&view);
        if (storage[1] != input[1] || storage[0] != 0xa5 || storage[2] != 0xa5) return 8;
    }
    return 0;
}
