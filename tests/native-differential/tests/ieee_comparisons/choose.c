#include <stdint.h>
#include <string.h>
extern uint64_t omega_entry(float, float, float);
static float decode(uint32_t bits) { float value; memcpy(&value, &bits, sizeof(value)); return value; }
int main(void) {
    const uint32_t cases[][4] = {
        {0x3f800000, 0x3f800000, 0x40000000, 7},
        {0x3f800000, 0x40000000, 0x3f800000, 9},
        {0x3f800000, 0x40000000, 0x40400000, 11},
        {0x3f800000, 0x3f800000, 0x3f800000, 7},
        {0x00000000, 0x80000000, 0x00000000, 7},
        {0x80000000, 0x3f800000, 0x00000000, 9},
        {0x7fc12345, 0x7fc12345, 0x7fc12345, 11},
        {0x3f800000, 0x7fc12345, 0x3f800000, 9},
        {0x7f800000, 0x7f800000, 0xff800000, 7},
        {0xff800000, 0x7f800000, 0xff800000, 9},
        {0x00000001, 0x00000001, 0x00000000, 7},
        {0x80000001, 0x00000001, 0x80000001, 9},
        {0x7f800001, 0x7f800001, 0x00000000, 11}
    };
    for (unsigned index = 0; index < sizeof(cases) / sizeof(cases[0]); ++index) {
        if (omega_entry(decode(cases[index][0]), decode(cases[index][1]), decode(cases[index][2])) != cases[index][3]) return (int)index + 1;
    }
    return 0;
}
