// The foreign worker models the compiler's private SysV entry including its
// reserved owner register. Its normal locals and return path use the fixed stack.
#include <stdalign.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef struct {
    uint32_t seed, starts, cleanups, result;
} Job;

enum { JOB_OFFSET = 256, CONTINUE = 1, STOP = 2 };
typedef struct {
    alignas(16) unsigned char control[JOB_OFFSET];
    Job work;
    unsigned char stack[TOP - JOB_OFFSET - sizeof(Job)];
    unsigned char guard[16];
} Backing;
_Static_assert(offsetof(Backing, guard) == TOP, "guard follows the complete stack");
extern int prepare(void *backing, uintptr_t length, uint32_t *job);
extern int step(Backing *owner);
extern int answer(Backing *owner, uint32_t *permit, uint32_t *value, uintptr_t bytes, uintptr_t kind);
extern void park_with_owner(Backing *owner, uint32_t *publication, uint32_t *reply);

static uint32_t *published_address(Backing *backing, unsigned offset) {
    uint32_t *address;
    memcpy(&address, backing->control + offset, sizeof address);
    return address;
}

void run_worker(Backing *backing) {
    Job *work = &backing->work;
    uint32_t local = 3 * work->seed + 7;
    uint32_t item = local + 1;
    uint32_t answer = 0;
    work->starts++;
    park_with_owner(backing, &item, &answer);
    if (answer == CONTINUE) {
        work->result = local + 100;
    } else if (answer == STOP) {
        work->result = local + 200;
    }
    work->cleanups++;
}

int main(void) {
    static Backing left, right;
    uint32_t first = 7, second = 17, stop = STOP, next = CONTINUE;
    static Backing unchanged;
    memset(&unchanged, 0xcd, sizeof unchanged);
    memcpy(&left, &unchanged, sizeof left);
    if (prepare(&left, TOP - 1, &first) || prepare(left.control + 1, TOP, &first) ||
        prepare(&left, UINTPTR_MAX, &first) || memcmp(&left, &unchanged, sizeof left)) return 9;
    static alignas(16) unsigned char unaligned[TOP + 32];
    memset(unaligned, 0x6b, sizeof unaligned);
    if (!prepare(unaligned + 1, TOP + 15, &first)) return 18;
    uintptr_t original, original_length;
    memcpy(&original, unaligned + 16 + BASE, sizeof original);
    memcpy(&original_length, unaligned + 16 + LENGTH, sizeof original_length);
    if (original != (uintptr_t)(unaligned + 1) || original_length != TOP + 15 ||
        unaligned[0] != 0x6b || unaligned[15] != 0x6b || unaligned[TOP + 16] != 0x6b) return 19;
    memcpy(&right, &unchanged, sizeof right);
    memset(&left.work, 0, sizeof(Job));
    memset(&right.work, 0, sizeof(Job));
    if (!prepare(&left, TOP + 9, &first) || !prepare(&right, TOP + 13, &second)) return 10;
    if (left.work.seed != first || right.work.seed != second || left.work.starts || right.work.starts) return 11;
    memset(left.guard, 0xa5, sizeof left.guard);
    memset(right.guard, 0xa5, sizeof right.guard);
    if (step(&left) != 1 || step(&right) != 1) return 1;
    uintptr_t left_slot = (uintptr_t)published_address(&left, PENDING);
    uintptr_t right_slot = (uintptr_t)published_address(&right, PENDING);
    if (left_slot < (uintptr_t)left.stack || left_slot >= (uintptr_t)left.guard ||
        right_slot < (uintptr_t)right.stack || right_slot >= (uintptr_t)right.guard) return 2;
    if (*published_address(&left, PENDING) != 29 || *published_address(&right, PENDING) != 59) return 3;
    if (answer(&left, (uint32_t *)right_slot, &next, sizeof next, 42) ||
        answer(&right, (uint32_t *)right_slot, &stop, sizeof stop, 41) ||
        answer(&right, (uint32_t *)right_slot, &stop, sizeof stop + 1, 42)) return 12;
    if (*published_address(&right, REPLY) || *published_address(&left, REPLY)) return 13;
    if (!answer(&right, (uint32_t *)right_slot, &stop, sizeof stop, 42) ||
        !answer(&left, (uint32_t *)left_slot, &next, sizeof next, 42) ||
        answer(&left, (uint32_t *)left_slot, &next, sizeof next, 42)) return 14;
    if (left.work.cleanups || right.work.cleanups) return 4;
    if (step(&right) != 0 || right.work.result != 258 || left.work.cleanups) return 5;
    if (step(&left) != 0 || left.work.result != 128) return 6;
    if (left.work.starts != 1 || right.work.starts != 1 ||
        left.work.cleanups != 1 || right.work.cleanups != 1) return 7;
    for (unsigned i = 0; i < sizeof left.guard; i++)
        if (left.guard[i] != 0xa5 || right.guard[i] != 0xa5) return 8;
    return 0;
}
