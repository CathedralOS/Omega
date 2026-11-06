#include <stdint.h>
#include <string.h>
static int neighbors(const unsigned char *table) {
    uint64_t before, after;
    memcpy(&before, table, 8); memcpy(&after, table + 536, 8);
    if (before != 101 || after != 203) return 0;
    for (unsigned i = 0; i < 33; ++i) {
        const unsigned char *gate = table + 8 + 16 * i;
        uint16_t selector; uint32_t reserved;
        memcpy(&selector, gate + 2, 2); memcpy(&reserved, gate + 12, 4);
        if (selector != 0x1234 || gate[4] != 5 || gate[5] != 0x8e || reserved != 0x89abcdef) return 0;
    }
    return 1;
}
uint64_t timer_register(const unsigned char *table) {
    if (!neighbors(table)) return 1;
    for (unsigned i = 0; i < 33; ++i) {
        const unsigned char *gate = table + 8 + 16 * i;
        uint16_t low, middle; uint32_t high;
        memcpy(&low, gate, 2); memcpy(&middle, gate + 6, 2); memcpy(&high, gate + 8, 4);
        uintptr_t address = (uintptr_t)low | ((uintptr_t)middle << 16) | ((uintptr_t)high << 32);
        if (!address) return 2;
        uint64_t value = 102 + i;
        uint64_t expected = value ^ (i == 32 ? 23 : 85);
        if (((uint64_t (*)(uint64_t))address)(value) != expected) return 3;
        if (!neighbors(table)) return 4;
    }
    return 0;
}
