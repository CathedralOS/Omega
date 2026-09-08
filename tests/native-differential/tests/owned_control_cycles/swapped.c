#include <stdint.h>
#include <unistd.h>

struct Payload { uint64_t first; uint64_t second; };
extern uint64_t omega_entry(uint64_t remaining, uint64_t marker,
                           struct Payload left, struct Payload right);

int main(void) {
    alarm(10);
    const struct Payload left = { UINT64_MAX, 13 };
    const struct Payload right = { 9123, 999 };
    for (uint64_t remaining = 0; remaining <= 5; ++remaining) {
        if (omega_entry(remaining, 876, left, right) != (remaining == 0 ? 876 : 1))
            return 1;
    }
    return 0;
}
