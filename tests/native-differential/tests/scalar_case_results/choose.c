#include <stdint.h>
#include <stddef.h>

struct outcome { uint32_t tag; uint32_t padding; uint64_t count; };
extern struct outcome omega_entry(uint64_t selector, uint64_t count);

int main(void) {
    const uint64_t counts[] = {0, 7, UINT32_MAX, UINT64_C(0x123456789abcdef0), UINT64_MAX};
    for (unsigned repeat = 0; repeat < 3; ++repeat) {
        for (uint64_t selector = 0; selector < 5; ++selector) {
            for (size_t position = 0; position < sizeof(counts) / sizeof(counts[0]); ++position) {
                struct outcome result = omega_entry(selector, counts[position]);
                uint32_t expected = selector < 3 ? (uint32_t)selector : 3;
                if (result.tag != expected) return 1;
                if (expected != 0 && result.count != counts[position]) return 2;
            }
        }
    }
    return 0;
}
