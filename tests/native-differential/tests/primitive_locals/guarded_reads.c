#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
extern void omega_entry(OMEGA_SCALAR replacement, OMEGA_SCALAR *output,
    const OMEGA_SCALAR *input, OMEGA_SCALAR *initial_output);
int main(void) {
    alarm(10);
    long page = sysconf(_SC_PAGESIZE);
    if (page <= 0) return 10;
    unsigned char *mapping = mmap(0, (size_t)page * 2, PROT_READ | PROT_WRITE,
        MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (mapping == MAP_FAILED) return 11;
    if (mprotect(mapping + page, (size_t)page, PROT_NONE) != 0) return 12;
    OMEGA_SCALAR *input = (OMEGA_SCALAR *)(mapping + page - sizeof(OMEGA_SCALAR));
    const uint64_t patterns[] = { OMEGA_PATTERNS };
    const size_t count = sizeof(patterns) / sizeof(patterns[0]);
    struct { unsigned char before[16]; OMEGA_SCALAR value; unsigned char after[16]; } output, initial_output;
    for (size_t position = 0; position < count; ++position) {
        OMEGA_SCALAR initial, replacement;
        memcpy(&initial, &patterns[(position + 1) % count], sizeof(initial));
        memcpy(input, &initial, sizeof(initial));
        memcpy(&replacement, &patterns[position], sizeof(replacement));
        memset(&output, 0xa5, sizeof(output));
        memset(&initial_output, 0xa5, sizeof(initial_output));
        memcpy(&output.value, &initial, sizeof(initial));
        memcpy(&initial_output.value, &replacement, sizeof(replacement));
        omega_entry(replacement, &output.value, input, &initial_output.value);
        if (memcmp(&output.value, &replacement, sizeof(replacement)) != 0) return 1;
        if (memcmp(&initial_output.value, &initial, sizeof(initial)) != 0) return 2;
        if (memcmp(input, &initial, sizeof(initial)) != 0) return 3;
        for (size_t byte = 0; byte < 16; ++byte) {
            if (output.before[byte] != 0xa5 || output.after[byte] != 0xa5
                || initial_output.before[byte] != 0xa5 || initial_output.after[byte] != 0xa5) return 4;
        }
    }
    return munmap(mapping, (size_t)page * 2) == 0 ? 0 : 13;
}
