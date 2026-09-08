#include <stdint.h>
#include <unistd.h>
extern uint64_t omega_entry(uint64_t remaining, uint64_t marker);
int main(void) {
    alarm(10);
    for (uint64_t remaining = 0; remaining <= 5; ++remaining) {
        if (omega_entry(remaining, 97) != (remaining ? 91 : 97)) return 1;
        if (omega_entry(remaining, UINT64_MAX) != (remaining ? 91 : UINT64_MAX)) return 2;
    }
    return 0;
}
