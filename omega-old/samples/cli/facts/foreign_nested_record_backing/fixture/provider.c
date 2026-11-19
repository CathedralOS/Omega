#include <stdint.h>
#include <stddef.h>
#if defined(_WIN32)
#define RETENTION_EXPORT __declspec(dllexport)
#else
#define RETENTION_EXPORT
#endif
struct cell { uint64_t value; };
struct buffer { uint64_t marker; struct cell cell; };
struct repeated_buffer { struct cell marker; struct cell cell; };
struct array_buffer { uint64_t marker; struct cell cells[3]; };
struct view_buffer { uint64_t marker; struct cell (*cells)[3]; };
_Static_assert(sizeof(struct view_buffer) == 16 && offsetof(struct view_buffer, cells) == 8,
               "a fixed-array reference is one pointer word beside the marker");
_Static_assert(sizeof(struct array_buffer) == 32 && offsetof(struct array_buffer, cells) == 8, "array root has ordinary repeated cell placement");
_Static_assert(sizeof(struct cell) == 8, "nested cell must use its ordinary u64 layout");
_Static_assert(sizeof(struct buffer) == 16, "Buffer is marker followed by a nested cell");
_Static_assert(offsetof(struct buffer, marker) == 0 && offsetof(struct buffer, cell) == 8,
               "C must use the same nested record field offsets");
_Static_assert(offsetof(struct cell, value) == 0, "nested scalar starts at cell offset zero");
_Static_assert(sizeof(struct repeated_buffer) == 16,
               "two occurrences of Cell retain the ordinary 16-byte record layout");
_Static_assert(offsetof(struct repeated_buffer, marker) == 0
               && offsetof(struct repeated_buffer, cell) == 8,
               "each repeated Cell field has its own parent path and layout offset");
enum state { VACANT, CAPACITY, REGISTERED };
enum retained_kind { NO_ROOT, ORIGINAL_ROOT, REPEATED_ROOT, ARRAY_ROOT, VIEW_ROOT };
union retained_root { struct buffer *original; struct repeated_buffer *repeated; struct array_buffer *array; struct view_buffer *view; };
static struct slot {
    enum state state;
    uint64_t capacity_generation;
    uint64_t registration_generation;
    enum retained_kind kind;
    union retained_root root;
} slots[2];
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
    slot->root.original = root;
    slot->kind = ORIGINAL_ROOT;
    slot->state = REGISTERED;
    return (slot->registration_generation << 2) | (capacity & 3);
}
RETENTION_EXPORT uint64_t nestlease_register_repeated(uint64_t capacity, struct repeated_buffer *root) {
    struct slot *slot = lookup(capacity, CAPACITY);
    if (root == NULL || root->marker.value != 91) invalid();
    if (slot->registration_generation == (UINT64_MAX >> 2)) invalid();
    slot->registration_generation++;
    root->cell.value = 41;
    slot->root.repeated = root;
    slot->kind = REPEATED_ROOT;
    slot->state = REGISTERED;
    return (slot->registration_generation << 2) | (capacity & 3);
}
RETENTION_EXPORT uint64_t nestlease_register_array(uint64_t capacity, struct array_buffer *root) {
    struct slot *slot = lookup(capacity, CAPACITY);
    if (root == NULL || root->marker != 91 || root->cells[0].value != 305419896 || root->cells[2].value != 1515863100) invalid();
    if (slot->registration_generation == (UINT64_MAX >> 2)) invalid();
    slot->registration_generation++;
    root->cells[1].value = 41;
    slot->root.array = root;
    slot->kind = ARRAY_ROOT;
    slot->state = REGISTERED;
    return (slot->registration_generation << 2) | (capacity & 3);
}
RETENTION_EXPORT uint64_t nestlease_register_view(uint64_t capacity, struct view_buffer *root) {
    struct slot *slot = lookup(capacity, CAPACITY);
    if (root == NULL || root->marker != 91 || root->cells == NULL
        || (*root->cells)[0].value != 305419896 || (*root->cells)[2].value != 1515863100) invalid();
    if (slot->registration_generation == (UINT64_MAX >> 2)) invalid();
    slot->registration_generation++;
    (*root->cells)[1].value = 41;
    slot->root.view = root;
    slot->kind = VIEW_ROOT;
    slot->state = REGISTERED;
    return (slot->registration_generation << 2) | (capacity & 3);
}
RETENTION_EXPORT uint64_t nestlease_complete(uint64_t pending, uint64_t *observed) {
    struct slot *slot = lookup(pending, REGISTERED);
    if (observed == NULL) invalid();
    switch (slot->kind) {
    case ORIGINAL_ROOT:
        if (slot->root.original->marker != 91) invalid();
        slot->root.original->cell.value = 73;
        *observed = slot->root.original->cell.value;
        break;
    case REPEATED_ROOT:
        if (slot->root.repeated->marker.value != 91) invalid();
        slot->root.repeated->cell.value = 73;
        *observed = slot->root.repeated->cell.value;
        break;
    case ARRAY_ROOT:
        if (slot->root.array->marker != 91 || slot->root.array->cells[0].value != 305419896 || slot->root.array->cells[2].value != 1515863100) invalid();
        slot->root.array->cells[1].value = 73;
        *observed = slot->root.array->cells[1].value;
        break;
    case VIEW_ROOT:
        if (slot->root.view->marker != 91 || slot->root.view->cells == NULL
            || (*slot->root.view->cells)[0].value != 305419896
            || (*slot->root.view->cells)[2].value != 1515863100) invalid();
        (*slot->root.view->cells)[1].value = 73;
        *observed = (*slot->root.view->cells)[1].value;
        break;
    default:
        invalid();
    }
    slot->root.original = NULL;
    slot->kind = NO_ROOT;
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
static void repeated_selftest(void) {
    uint64_t first_capacity = nestlease_acquire(), second_capacity = nestlease_acquire();
    struct buffer original = {91, {37}};
    struct repeated_buffer repeated = {{91}, {37}};
    uint64_t observed = 0;
    uint64_t first = nestlease_register(first_capacity, &original);
    uint64_t second = nestlease_register_repeated(second_capacity, &repeated);
    assert(original.marker == 91 && original.cell.value == 41);
    assert(repeated.marker.value == 91 && repeated.cell.value == 41);
    assert(nestlease_complete(second, &observed) == second_capacity && observed == 73);
    assert(repeated.marker.value == 91 && repeated.cell.value == 73);
    assert(original.marker == 91 && original.cell.value == 41);
    assert(nestlease_complete(first, &observed) == first_capacity && observed == 73);
    uint64_t next = nestlease_register_repeated(first_capacity, &repeated);
    assert(next != first && repeated.marker.value == 91 && repeated.cell.value == 41);
    assert(nestlease_complete(next, &observed) == first_capacity && observed == 73);
    assert(repeated.marker.value == 91 && repeated.cell.value == 73);
    nestlease_release(first_capacity);
    nestlease_release(second_capacity);
}
static void array_selftest(void) {
    uint64_t capacity = nestlease_acquire(), observed = 0;
    struct array_buffer value = {91, {{305419896}, {37}, {1515863100}}};
    uint64_t pending = nestlease_register_array(capacity, &value);
    assert(value.marker == 91 && value.cells[1].value == 41);
    assert(value.cells[0].value == 305419896 && value.cells[2].value == 1515863100);
    assert(nestlease_complete(pending, &observed) == capacity && observed == 73);
    assert(value.marker == 91 && value.cells[1].value == 73);
    assert(value.cells[0].value == 305419896 && value.cells[2].value == 1515863100);
    nestlease_release(capacity);
}
static void view_selftest(void) {
    uint64_t first_capacity = nestlease_acquire(), second_capacity = nestlease_acquire(), observed = 0;
    struct cell first_cells[3] = {{305419896}, {37}, {1515863100}};
    struct cell second_cells[3] = {{305419896}, {57}, {1515863100}};
    struct view_buffer first = {91, &first_cells}, second = {91, &second_cells};
    uint64_t first_pending = nestlease_register_view(first_capacity, &first);
    uint64_t second_pending = nestlease_register_view(second_capacity, &second);
    assert(first_cells[1].value == 41 && second_cells[1].value == 41);
    assert(nestlease_complete(second_pending, &observed) == second_capacity && observed == 73);
    assert(first_cells[1].value == 41 && second_cells[1].value == 73);
    assert(nestlease_complete(first_pending, &observed) == first_capacity && observed == 73);
    assert(first.cells == &first_cells && second.cells == &second_cells);
    assert(first.marker == 91 && second.marker == 91 && first_cells[1].value == 73);
    assert(first_cells[0].value == 305419896 && first_cells[2].value == 1515863100);
    assert(second_cells[0].value == 305419896 && second_cells[2].value == 1515863100);
    nestlease_release(first_capacity); nestlease_release(second_capacity);
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
    repeated_selftest();
    array_selftest();
    view_selftest();
    return 0;
}
#endif
