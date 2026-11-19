#include <stdint.h>
#include <stddef.h>
#if defined(_WIN32)
#define RETENTION_EXPORT __declspec(dllexport)
#else
#define RETENTION_EXPORT
#endif
enum state { VACANT, CAPACITY, REGISTERED };
static struct slot { enum state state; uint64_t capacity_generation; uint64_t registration_generation; const uint64_t *root; } slots[2];
static void invalid(void) { __builtin_trap(); }
static struct slot *lookup(uint64_t token, enum state state) {
    uint64_t index = token & 3;
    if (index < 1 || index > 2) invalid();
    struct slot *slot = &slots[index - 1];
    uint64_t generation = state == REGISTERED ? slot->registration_generation : slot->capacity_generation;
    if (slot->state != state || generation != (token >> 2)) invalid();
    return slot;
}
RETENTION_EXPORT uint64_t retention_acquire(void) {
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
RETENTION_EXPORT uint64_t retention_register(uint64_t capacity, const uint64_t *root) {
    struct slot *slot = lookup(capacity, CAPACITY);
    if (root == NULL) invalid();
    if (slot->registration_generation == (UINT64_MAX >> 2)) invalid();
    slot->registration_generation++;
    slot->root = root;
    slot->state = REGISTERED;
    return (slot->registration_generation << 2) | (capacity & 3);
}
RETENTION_EXPORT uint64_t retention_complete(uint64_t pending, uint64_t *observed) {
    struct slot *slot = lookup(pending, REGISTERED);
    if (observed == NULL) invalid();
    *observed = *slot->root;
    slot->root = NULL;
    slot->state = CAPACITY;
    return (slot->capacity_generation << 2) | (pending & 3);
}
RETENTION_EXPORT void retention_release(uint64_t capacity) { lookup(capacity, CAPACITY)->state = VACANT; }
RETENTION_EXPORT uint64_t retention_same(const uint64_t *first, const uint64_t *second) {
    if (first == NULL || second == NULL) invalid();
    return first == second && *first == *second ? 0 : 1;
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
    uint64_t capacity = retention_acquire(), value = 37, observed = 0;
    if (strcmp(name, "exhausted") == 0) {
        retention_acquire(); retention_acquire();
    } else if (strcmp(name, "null_root") == 0) {
        retention_register(capacity, NULL);
    } else if (strcmp(name, "stale") == 0) {
        retention_release(capacity); retention_acquire();
        retention_register(capacity, &value);
    } else if (strcmp(name, "duplicate_register") == 0) {
        retention_register(capacity, &value); retention_register(capacity, &value);
    } else if (strcmp(name, "duplicate_complete") == 0) {
        uint64_t pending = retention_register(capacity, &value);
        retention_complete(pending, &observed); retention_complete(pending, &observed);
    } else if (strcmp(name, "release_live") == 0) {
        retention_register(capacity, &value); retention_release(capacity);
    } else if (strcmp(name, "null_output") == 0) {
        retention_complete(retention_register(capacity, &value), NULL);
    } else if (strcmp(name, "duplicate_release") == 0) {
        retention_release(capacity); retention_release(capacity);
    } else if (strcmp(name, "stale_pending") == 0) {
        uint64_t old = retention_register(capacity, &value);
        retention_complete(old, &observed); retention_register(capacity, &value);
        retention_complete(old, &observed);
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
    uint64_t a = retention_acquire(), b = retention_acquire();
    assert(a != b);
    uint64_t x = 37, y = 91, observed = 0;
    assert(retention_same(&x, &x) == 0 && retention_same(&x, &y) == 1);
    uint64_t first = retention_register(a, &x), second = retention_register(b, &y);
    assert(retention_complete(first, &observed) == a && observed == 37);
    assert(retention_complete(second, &observed) == b && observed == 91);
    uint64_t next = retention_register(a, &x);
    assert(next != first);
    assert(retention_complete(next, &observed) == a && observed == 37);
    retention_release(a); retention_release(b);
    uint64_t c = retention_acquire();
    assert(c != a && c != b);
    retention_release(c);
    return 0;
}
#endif
