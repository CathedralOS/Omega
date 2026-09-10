#include <stdint.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>
/* The C entry uses only a register argument. Ten-argument stack transport is
 * between generated Omega functions using their retained internal call policy. */
extern FLOAT_TYPE omega_entry(FLOAT_TYPE);
static FLOAT_TYPE decode(BITS_TYPE bits) {
    FLOAT_TYPE value;
    memcpy(&value, &bits, sizeof(value));
    return value;
}
int main(void) {
#if FORMAT_BITS == 32
    const BITS_TYPE cases[] = {
        0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000,
        0x3f800000, 0xbf800000, 0x7f800000, 0xff800000,
        0x7fc12345, 0xffc54321, 0x7f800001, 0xff800001
    };
#else
    const BITS_TYPE cases[] = {
        0, UINT64_C(0x8000000000000000), 1, UINT64_C(0x8000000000000001),
        UINT64_C(0x000fffffffffffff), UINT64_C(0x0010000000000000),
        UINT64_C(0x3ff0000000000000), UINT64_C(0xbff0000000000000),
        UINT64_C(0x7ff0000000000000), UINT64_C(0xfff0000000000000),
        UINT64_C(0x7ff8123456789abc), UINT64_C(0xfff8cba987654321),
        UINT64_C(0x7ff0000000000001), UINT64_C(0xfff0000000000001)
    };
#endif
    const unsigned count = sizeof(cases) / sizeof(cases[0]);
    for (unsigned index = 0; index < count; ++index) {
        FLOAT_TYPE result = omega_entry(decode(cases[index]));
        BITS_TYPE actual;
        memcpy(&actual, &result, sizeof(actual));
        if (actual != cases[index]) {
            fprintf(stderr, "binary%d ten-argument return case %u: expected 0x%016" PRIx64 ", actual 0x%016" PRIx64 "\n",
                FORMAT_BITS, index, (uint64_t)cases[index], (uint64_t)actual);
            return (int)index + 1;
        }
    }
    return 0;
}
