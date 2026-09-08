#include <stdint.h>
#include <unistd.h>
extern void omega_entry(uint64_t replacement, uint64_t returned, uint64_t *output, uint64_t *return_output);
int main(void) {
    alarm(10);
    uint64_t words[] = { 123, 456, 789 };
    uint64_t result_words[] = { 321, 654, 987 };
    const uint64_t values[] = { 0, 1, 91, 123456789, UINT64_MAX };
    for (unsigned position = 0; position < sizeof(values) / sizeof(values[0]); ++position) {
        uint64_t returned = values[position] ^ UINT64_MAX;
        omega_entry(values[position], returned, &words[1], &result_words[1]);
        if (result_words[1] != returned) return 1;
        if (words[1] != values[position]) return 2;
        if (words[0] != 123 || words[2] != 789) return 3;
        if (result_words[0] != 321 || result_words[2] != 987) return 4;
    }
    return 0;
}
