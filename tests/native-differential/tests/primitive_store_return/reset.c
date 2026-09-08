#include <stdint.h>
#include <unistd.h>

extern uint64_t omega_entry(uint64_t *value);

int main(void) {
    alarm(10);
    const uint64_t inputs[] = {1, 17, UINT64_C(0x8000000000000000), UINT64_MAX};
    for (unsigned repetition = 0; repetition < 3; ++repetition) {
        for (unsigned position = 0; position < sizeof(inputs) / sizeof(inputs[0]); ++position) {
            uint64_t words[] = {UINT64_C(0x0123456789abcdef), inputs[position], UINT64_C(0xfedcba9876543210)};
            for (unsigned call = 0; call < 3; ++call) {
                words[1] = inputs[position];
                if (omega_entry(&words[1]) != 0) return 1;
                if (words[1] != 0) return 2;
                if (words[0] != UINT64_C(0x0123456789abcdef)) return 3;
                if (words[2] != UINT64_C(0xfedcba9876543210)) return 4;
            }
        }
    }
    return 0;
}
