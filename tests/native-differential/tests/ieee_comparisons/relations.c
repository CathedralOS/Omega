#include <stdbool.h>
#include <stdint.h>
#include <string.h>
extern bool omega_entry(FLOAT_TYPE, FLOAT_TYPE);
static FLOAT_TYPE decode(BITS_TYPE bits) { FLOAT_TYPE value; memcpy(&value, &bits, sizeof(value)); return value; }
int main(void) {
#if FORMAT_BITS == 32
    const BITS_TYPE cases[] = {0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000,
        0x3f800000, 0xbf800000, 0x40000000, 0xc0000000, 0x7f7fffff, 0xff7fffff,
        0x7f800000, 0xff800000, 0x7fc12345, 0xffc12345, 0x7f800001, 0xff800001};
#else
    const BITS_TYPE cases[] = {0, UINT64_C(0x8000000000000000), 1, UINT64_C(0x8000000000000001),
        UINT64_C(0x000fffffffffffff), UINT64_C(0x0010000000000000),
        UINT64_C(0x3ff0000000000000), UINT64_C(0xbff0000000000000), UINT64_C(0x4000000000000000), UINT64_C(0xc000000000000000),
        UINT64_C(0x7fefffffffffffff), UINT64_C(0xffefffffffffffff), UINT64_C(0x7ff0000000000000), UINT64_C(0xfff0000000000000),
        UINT64_C(0x7ff8123456789abc), UINT64_C(0xfff8123456789abc), UINT64_C(0x7ff0000000000001), UINT64_C(0xfff0000000000001)};
#endif
    for (unsigned left = 0; left < sizeof(cases) / sizeof(cases[0]); ++left) {
        for (unsigned right = 0; right < sizeof(cases) / sizeof(cases[0]); ++right) {
            FLOAT_TYPE a = decode(cases[left]), b = decode(cases[right]);
            if (omega_entry(a, b) != (a COMPARE_OPERATOR b)) return 1;
        }
    }
    return 0;
}
