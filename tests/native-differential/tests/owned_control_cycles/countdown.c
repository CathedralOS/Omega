#include <stdint.h>
#include <unistd.h>

struct Limits { uint64_t limit; uint64_t divisor; };
extern uint64_t omega_entry(uint64_t remaining, uint64_t marker, struct Limits limits);

int main(void) {
    alarm(10);
    const struct Limits limits = { UINT64_MAX, 3 };
    for (uint64_t remaining = 0; remaining <= 5; ++remaining) {
        uint64_t expected = remaining == 0 ? 97 : 1;
        if (omega_entry(remaining, 97, limits) != expected) return 1;
        if (omega_entry(remaining, 123456789, limits) !=
            (remaining == 0 ? 123456789 : 1)) return 2;
    }
    return 0;
}
