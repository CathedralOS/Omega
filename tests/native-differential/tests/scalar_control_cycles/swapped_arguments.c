#include <stdint.h>
#include <unistd.h>

extern uint64_t omega_entry(uint64_t remaining, uint64_t first, uint64_t second);

int main(void) {
    alarm(10);
    const uint64_t values[] = {0, 1, 17, 29, UINT64_C(0x8000000000000000), UINT64_MAX};
    for (unsigned repetition = 0; repetition < 3; ++repetition) {
        for (unsigned first_position = 0; first_position < 6; ++first_position) {
            for (unsigned second_position = 0; second_position < 6; ++second_position) {
                uint64_t first = values[first_position], second = values[second_position];
                for (uint64_t remaining = 0; remaining <= 5; ++remaining) {
                    uint64_t expected = remaining % 2 == 0 ? first : second;
                    if (omega_entry(remaining, first, second) != expected)
                        return 1;
                }
            }
        }
    }
    return 0;
}
