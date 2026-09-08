#include <stdint.h>
#include <unistd.h>
extern uint64_t omega_entry(uint64_t replacement);
int main(void) {
    alarm(10);
    const uint64_t values[] = { 0, 1, 91, 123456789, UINT64_MAX };
    for (unsigned position = 0; position < sizeof(values) / sizeof(values[0]); ++position) {
        if (omega_entry(values[position]) != values[position]) return 1;
    }
    return 0;
}
