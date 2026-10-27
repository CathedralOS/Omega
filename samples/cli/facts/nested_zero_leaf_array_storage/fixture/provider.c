#include <stdint.h>
#if defined(_WIN32)
#define EXPORT __declspec(dllexport)
#else
#define EXPORT __attribute__((visibility("default")))
#endif
typedef struct Packet { uint64_t before; uint64_t after; } Packet;
_Static_assert(sizeof(Packet) == 16, "only two stored scalar lanes");
EXPORT Packet zero_leaf_make(void) { return (Packet){17, 31}; }
EXPORT Packet zero_leaf_shift(Packet packet) {
    return (Packet){packet.before + 11, packet.after + 29};
}
#ifdef ZERO_LEAF_FIXTURE_TEST
int main(void) {
    Packet first = zero_leaf_make();
    Packet shifted = zero_leaf_shift(first);
    return !(first.before == 17 && first.after == 31 && shifted.before == 28 && shifted.after == 60);
}
#endif
