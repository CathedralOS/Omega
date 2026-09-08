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
    const uint8_t raw[] = { 2, 0xff, 1, 0x80 };
    struct ByteView view = { raw, sizeof(raw) };
    /* An unselected call must not even read the absent descriptor. */
    if (omega_entry(1, 0, NULL) != 777) return 1;
    if (omega_entry(UINT64_MAX, UINT64_MAX, NULL) != 777) return 2;
    for (uint64_t position = 0; position <= sizeof(raw); ++position) {
        const uint64_t first = read_byte(position, &view);
        if (omega_entry(0, position, &view) != first + read_byte(first, &view)) return 3;
    }
    if (omega_entry(0, UINT64_MAX, &view) != 512) return 4;
    if (view.bytes != raw || view.length != sizeof(raw)) return 5;
    view.bytes = NULL; view.length = 0;
    if (omega_entry(0, 0, &view) != 512) return 6;
    if (omega_entry(1, 0, &view) != 777) return 7;
    return 0;
}
