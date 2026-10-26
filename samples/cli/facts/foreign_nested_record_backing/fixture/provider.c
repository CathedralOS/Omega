#include <stdint.h>
#include <stddef.h>
#if defined(_WIN32)
#define RETENTION_EXPORT __declspec(dllexport)
#else
#define RETENTION_EXPORT
#endif
struct cell { uint64_t value; };
struct buffer { uint64_t marker; struct cell cell; };
_Static_assert(sizeof(struct cell) == 8, "nested cell must use its ordinary u64 layout");
_Static_assert(sizeof(struct buffer) == 16, "Buffer is marker followed by a nested cell");
_Static_assert(offsetof(struct buffer, marker) == 0 && offsetof(struct buffer, cell) == 8,
               "C must use the same nested record field offsets");
_Static_assert(offsetof(struct cell, value) == 0, "nested scalar starts at cell offset zero");
enum state { VACANT, CAPACITY, REGISTERED };
static struct slot { enum state state; uint64_t capacity_generation; uint64_t registration_generation; struct buffer *root; } slots[2];
static void invalid(void) { __builtin_trap(); }
static struct slot *lookup(uint64_t token, enum state state) {
    uint64_t index = token & 3;
    if (index < 1 || index > 2) invalid();
    struct slot *slot = &slots[index - 1];
    uint64_t generation = state == REGISTERED ? slot->registration_generation : slot->capacity_generation;
    if (slot->state != state || generation != (token >> 2)) invalid();
    return slot;
}
RETENTION_EXPORT uint64_t nestlease_acquire(void) {
    for (size_t i = 0; i < 2; i++) {
        struct slot *slot = &slots[i];
        if (slot->state != VACANT) continue;
        if (slot->capacity_generation == (UINT64_MAX >> 2)) invalid();
        slot->capacity_generation++;
        slot->state = CAPACITY;
        return (slot->capacity_generation << 2) | (i + 1);
    }
    invalid();
    return 0;
}
RETENTION_EXPORT uint64_t nestlease_register(uint64_t capacity, struct buffer *root) {
    struct slot *slot = lookup(capacity, CAPACITY);
    if (root == NULL || root->marker != 91) invalid();
    if (slot->registration_generation == (UINT64_MAX >> 2)) invalid();
    slot->registration_generation++;
    root->cell.value = 41;
    slot->root = root;
    slot->state = REGISTERED;
    return (slot->registration_generation << 2) | (capacity & 3);
}
RETENTION_EXPORT uint64_t nestlease_complete(uint64_t pending, uint64_t *observed) {
    struct slot *slot = lookup(pending, REGISTERED);
    if (observed == NULL) invalid();
    if (slot->root->marker != 91) invalid();
    slot->root->cell.value = 73;
    *observed = slot->root->cell.value;
    slot->root = NULL;
    slot->state = CAPACITY;
    return (slot->capacity_generation << 2) | (pending & 3);
}
RETENTION_EXPORT void nestlease_release(uint64_t capacity) { lookup(capacity, CAPACITY)->state = VACANT; }
RETENTION_EXPORT uint64_t nestlease_same(const struct buffer *first, const struct buffer *second) {
    if (first == NULL || second == NULL) invalid();
    return first == second && first->marker == second->marker && first->cell.value == second->cell.value ? 0 : 1;
}
#ifdef RETENTION_FIXTURE_TEST
#include <assert.h>
#include <string.h>
#if defined(_WIN32)
#include <windows.h>
#else
#include <sys/resource.h>
#endif
static int invalid_control(const char *name) {
    uint64_t capacity = nestlease_acquire(), observed = 0;
    struct buffer value = {91, {37}};
    if (strcmp(name, "exhausted") == 0) {
        nestlease_acquire(); nestlease_acquire();
    } else if (strcmp(name, "null_root") == 0) {
        nestlease_register(capacity, NULL);
    } else if (strcmp(name, "stale") == 0) {
        nestlease_release(capacity); nestlease_acquire();
        nestlease_register(capacity, &value);
    } else if (strcmp(name, "duplicate_register") == 0) {
        nestlease_register(capacity, &value); nestlease_register(capacity, &value);
    } else if (strcmp(name, "duplicate_complete") == 0) {
        uint64_t pending = nestlease_register(capacity, &value);
        nestlease_complete(pending, &observed); nestlease_complete(pending, &observed);
    } else if (strcmp(name, "release_live") == 0) {
        nestlease_register(capacity, &value); nestlease_release(capacity);
    } else if (strcmp(name, "null_output") == 0) {
        nestlease_complete(nestlease_register(capacity, &value), NULL);
    } else if (strcmp(name, "duplicate_release") == 0) {
        nestlease_release(capacity); nestlease_release(capacity);
    } else if (strcmp(name, "stale_pending") == 0) {
        uint64_t old = nestlease_register(capacity, &value);
        nestlease_complete(old, &observed); nestlease_register(capacity, &value);
        nestlease_complete(old, &observed);
    }
    return 1; /* Neither an unknown case nor a normal return proves a trap. */
}
int main(int argc, char **argv) {
#if defined(_WIN32)
    SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
#else
    struct rlimit no_core = {0, 0};
    assert(setrlimit(RLIMIT_CORE, &no_core) == 0);
#endif
    if (argc == 2) return invalid_control(argv[1]);
    if (argc != 1) return 1;
    uint64_t a = nestlease_acquire(), b = nestlease_acquire();
    assert(a != b);
    struct buffer x = {91, {37}}, y = {91, {57}};
    uint64_t observed = 0;
    assert(nestlease_same(&x, &x) == 0 && nestlease_same(&x, &y) == 1);
    uint64_t first = nestlease_register(a, &x), second = nestlease_register(b, &y);
    assert(x.cell.value == 41 && y.cell.value == 41 && x.marker == 91 && y.marker == 91);
    assert(nestlease_complete(first, &observed) == a && observed == 73);
    assert(nestlease_complete(second, &observed) == b && observed == 73);
    assert(x.cell.value == 73 && y.cell.value == 73 && x.marker == 91 && y.marker == 91);
    uint64_t next = nestlease_register(a, &x);
    assert(next != first);
    assert(nestlease_complete(next, &observed) == a && observed == 73);
    nestlease_release(a); nestlease_release(b);
    uint64_t c = nestlease_acquire();
    assert(c != a && c != b);
    nestlease_release(c);
    return 0;
}
#endif
