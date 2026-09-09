#include <stdint.h>
#include <stddef.h>
#include <string.h>

struct byte_view { uint8_t *bytes; uint64_t length; };
extern void omega_entry(uint64_t selector, uint64_t count, struct byte_view *out);

int main(void) {
    const uint64_t counts[] = {0, 7, UINT32_MAX, UINT64_C(0x123456789abcdef0), UINT64_MAX};
    for (unsigned repeat = 0; repeat < 3; ++repeat) {
        for (uint64_t selector = 0; selector < 5; ++selector) {
            for (size_t position = 0; position < sizeof(counts) / sizeof(counts[0]); ++position) {
                for (uint64_t length = 0; length < 5; ++length) {
                    uint8_t backing[8];
                    memset(backing, 0xa7, sizeof(backing));
                    struct byte_view out = {backing + 1, length};
                    omega_entry(selector, counts[position], &out);
                    if (out.bytes != backing + 1 || out.length != length) return 1;
                    for (size_t byte = 0; byte < sizeof(backing); ++byte) {
                        uint8_t expected = length && byte == 1
                            ? (selector < 3 ? (uint8_t)(selector * 11) : 33) : 0xa7;
#ifdef CALLEE_FILLS_VIEW
                        if (byte > 1 && byte <= length) expected = 37;
#endif
                        if (backing[byte] != expected) return 2;
                    }
                }
            }
        }
    }
    return 0;
}
