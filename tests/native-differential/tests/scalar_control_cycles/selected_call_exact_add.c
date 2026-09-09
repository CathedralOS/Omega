#include <stdint.h>
#include <unistd.h>

extern uint64_t omega_entry(uint64_t remaining, uint64_t marker);

int main(void) {
    alarm(10);
    const uint64_t markers[] = {0, 17, 29, UINT64_C(0x8000000000000000), UINT64_MAX};
    for (unsigned repetition = 0; repetition < 3; ++repetition) {
        for (unsigned marker_position = 0; marker_position < 5; ++marker_position) {
            for (uint64_t remaining = 0; remaining <= 5; ++remaining) {
                uint64_t expected = remaining == 0 ? markers[marker_position] : 2;
                if (omega_entry(remaining, markers[marker_position]) != expected)
                    return 1;
            }
        }
    }
    return 0;
}
