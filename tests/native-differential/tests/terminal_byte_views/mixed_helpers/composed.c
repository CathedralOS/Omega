#include <stdint.h>
#include <stddef.h>
#include <unistd.h>
struct ByteView { const uint8_t *bytes; uint64_t length; };
extern uint64_t omega_entry(uint64_t, uint64_t, const struct ByteView *);
static uint64_t read_byte(uint64_t position, const struct ByteView *view) {
    return position < view->length ? view->bytes[position] : 256;
}
int main(void) {
    alarm(10);
    uint8_t raw[] = { 2, 0xff, 3, 0, 0x80, 1 };
    const uint8_t other[] = { 1, 0, 0xfe };
    const uint64_t positions[] = { 0, 1, 2, 3, 4, 5, 6, UINT64_MAX };
    struct ByteView view = { raw, sizeof(raw) };
    for (unsigned configuration = 0; configuration < 4; ++configuration) {
        if (configuration == 1) raw[2] = 5;
        if (configuration == 2) { view.bytes = other; view.length = sizeof(other); }
        if (configuration == 3) { view.bytes = NULL; view.length = 0; }
        const uint8_t *original = view.bytes;
        const uint64_t length = view.length;
        for (unsigned first = 0; first < 8; ++first) {
            for (unsigned second = 0; second < 8; ++second) {
                const uint64_t first_result = read_byte(positions[second], &view);
                const uint64_t expected = first_result + read_byte(positions[first], &view)
                    + read_byte(first_result, &view);
                if (omega_entry(positions[first], positions[second], &view) != expected) return 1;
                if (view.bytes != original || view.length != length) return 2;
            }
        }
    }
    return 0;
}
