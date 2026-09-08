#include <stdint.h>
#include <unistd.h>
struct Limits { uint64_t limit; uint64_t divisor; };
extern uint64_t omega_entry(uint64_t remaining, uint64_t marker, struct Limits limits);
int main(void) {
    alarm(10);
    for (uint64_t divisor = 3; divisor <= 5; ++divisor) {
        const struct Limits limits = { UINT64_MAX, divisor };
        for (uint64_t remaining = 0; remaining <= 5; ++remaining) {
            if (omega_entry(remaining, 97, limits) != 0) return 1;
            if (omega_entry(remaining, UINT64_MAX, limits) != 0) return 2;
        }
    }
    return 0;
}
