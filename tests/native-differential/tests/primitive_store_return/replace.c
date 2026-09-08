#include <stdint.h>
#include <unistd.h>

/* Omega's native signature places scalar parameters before structural pointers. */
extern uint64_t omega_entry(uint64_t replacement, uint64_t returned, uint64_t *value);

int main(void) {
    alarm(10);
    const uint64_t values[] = {0, 1, 17, UINT64_C(0x8000000000000000), UINT64_MAX};
    for (unsigned repetition = 0; repetition < 3; ++repetition) {
        for (unsigned stored = 0; stored < sizeof(values) / sizeof(values[0]); ++stored) {
            for (unsigned returned = 0; returned < sizeof(values) / sizeof(values[0]); ++returned) {
                uint64_t words[] = {UINT64_C(0x0123456789abcdef), ~values[stored], UINT64_C(0xfedcba9876543210)};
                if (omega_entry(values[stored], values[returned], &words[1]) != values[returned]) return 1;
                if (words[1] != values[stored]) return 2;
                if (words[0] != UINT64_C(0x0123456789abcdef)) return 3;
                if (words[2] != UINT64_C(0xfedcba9876543210)) return 4;
            }
        }
    }
    return 0;
}
