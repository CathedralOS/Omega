#include <assert.h>
#include <stdint.h>
#include <stddef.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <sys/resource.h>
#endif

struct frame { const void *const *arguments; void *result; };
struct small { uint64_t first, second; };
struct large { uint64_t first, second, third; };
extern void bridge_retention_acquire(struct frame *);
extern void bridge_retention_register(struct frame *);
extern void bridge_retention_complete(struct frame *);
extern void bridge_retention_release(struct frame *);
extern void bridge_stack_words(struct frame *);
extern void bridge_stack_floats(struct frame *);
extern void bridge_small_record(struct frame *);
extern void bridge_large_record(struct frame *);
extern void bridge_narrow_scalars_and_records(struct frame *);
struct byte_record { int8_t value; };
struct short_record { int16_t value; };
int64_t narrow_scalars_and_records(int8_t a, int16_t b, struct byte_record c,
                                  struct short_record d, uint8_t e, uint16_t f) {
    return (int64_t)a + b + c.value + d.value + e + f;
}

uint64_t stack_words(uint64_t a, uint64_t b, uint64_t c, uint64_t d,
                     uint64_t e, uint64_t f, uint64_t g, uint64_t h,
                     uint64_t i, uint64_t j, uint64_t k, uint64_t l) {
    return a + 2*b + 3*c + 4*d + 5*e + 6*f + 7*g + 8*h + 9*i + 10*j + 11*k + 12*l;
}
double stack_floats(double a, double b, double c, double d, double e,
                    double f, double g, double h, double i, double j) {
    return a + 2*b + 3*c + 4*d + 5*e + 6*f + 7*g + 8*h + 9*i + 10*j;
}
struct small small_record(struct small input, uint64_t increment) {
    return (struct small){input.second + increment, input.first + 2*increment};
}
struct large large_record(struct large input, uint64_t increment) {
    return (struct large){input.third + increment, input.first + 2*increment, input.second + 3*increment};
}
int main(void) {
#ifdef _WIN32
    SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
#else
    const struct rlimit core_limit = {0, 0};
    assert(setrlimit(RLIMIT_CORE, &core_limit) == 0);
#endif
    uint64_t capacity = 0, pending = 0, returned = 0, root = 37, observed = 0;
    struct frame frame = {NULL, &capacity};
    bridge_retention_acquire(&frame);
    const uint64_t *root_pointer = &root;
    const void *registration[] = {&capacity, &root_pointer};
    frame = (struct frame){registration, &pending};
    bridge_retention_register(&frame);
    assert(capacity != 0 && pending != 0);
    // A physical-layer probe deliberately changes the C root between calls.
    // Completion must dereference that exact live root, not an argument snapshot.
    root = 91;
    uint64_t *observed_pointer = &observed;
    const void *completion[] = {&pending, &observed_pointer};
    frame = (struct frame){completion, &returned};
    bridge_retention_complete(&frame);
    assert(returned == capacity && observed == 91 && root == 91);
    const void *release[] = {&returned};
    frame = (struct frame){release, NULL};
    bridge_retention_release(&frame);

    uint64_t words[12], word_result = 0;
    const void *arguments[12];
    for (size_t n = 0; n < 12; n++) { words[n] = n + 1; arguments[n] = &words[n]; }
    frame = (struct frame){arguments, &word_result};
    bridge_stack_words(&frame);
    assert(word_result == 650);
    double floats[10], float_result = 0;
    for (size_t n = 0; n < 10; n++) { floats[n] = n + 1; arguments[n] = &floats[n]; }
    frame = (struct frame){arguments, &float_result};
    bridge_stack_floats(&frame);
    assert(float_result == 385);

    uint64_t increment = 5;
    struct small small_input = {17, 23}, small_output = {0, 0};
    arguments[0] = &small_input; arguments[1] = &increment;
    frame = (struct frame){arguments, &small_output};
    bridge_small_record(&frame);
    assert(small_output.first == 28 && small_output.second == 27);
    struct large large_input = {11, 13, 19}, large_output = {0, 0, 0};
    arguments[0] = &large_input;
    frame = (struct frame){arguments, &large_output};
    bridge_large_record(&frame);
    assert(large_output.first == 24 && large_output.second == 21 && large_output.third == 28);
    assert(large_input.first == 11 && large_input.second == 13 && large_input.third == 19);
    int8_t a = -7; int16_t b = -300;
    struct byte_record c = {-11}; struct short_record d = {-2000};
    uint8_t e = 249; uint16_t f = 65530;
    int64_t narrow_result = 0;
    arguments[0] = &a; arguments[1] = &b; arguments[2] = &c;
    arguments[3] = &d; arguments[4] = &e; arguments[5] = &f;
    frame = (struct frame){arguments, &narrow_result};
    bridge_narrow_scalars_and_records(&frame);
    assert(narrow_result == 63461);
    return 0;
}
