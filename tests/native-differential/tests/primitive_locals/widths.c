#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <unistd.h>
extern void omega_entry(OMEGA_SCALAR initial, OMEGA_SCALAR replacement, OMEGA_SCALAR *output);
int main(void) {
    alarm(10);
    const uint64_t patterns[] = { OMEGA_PATTERNS };
    const size_t count = sizeof(patterns) / sizeof(patterns[0]);
    struct { unsigned char before[16]; OMEGA_SCALAR value; unsigned char after[16]; } output;
    for (size_t position = 0; position < count; ++position) {
        OMEGA_SCALAR initial, replacement;
        memcpy(&initial, &patterns[(position + 1) % count], sizeof(initial));
        memcpy(&replacement, &patterns[position], sizeof(replacement));
        memset(&output, 0xa5, sizeof(output));
        memcpy(&output.value, &initial, sizeof(initial));
        omega_entry(initial, replacement, &output.value);
        if (memcmp(&output.value, &replacement, sizeof(replacement)) != 0) return 1;
        for (size_t byte = 0; byte < 16; ++byte) {
            if (output.before[byte] != 0xa5 || output.after[byte] != 0xa5) return 2;
        }
    }
    return 0;
}
