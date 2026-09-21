#include <stdint.h>
#include <unistd.h>
extern uint64_t omega_entry(uint64_t seed, uint64_t *cell);
int main(void) {
    alarm(10);
    const uint64_t values[] = { 0, 1, 91, 123456789, UINT64_MAX };
    for (unsigned position = 0; position < sizeof(values) / sizeof(values[0]); ++position) {
        uint64_t words[] = { 123, 456, 789 };
        if (omega_entry(values[position], &words[1]) != values[position]) return 1;
        if (words[1] != values[position]) return 2;
        if (words[0] != 123 || words[2] != 789) return 3;
    }
    return 0;
}
