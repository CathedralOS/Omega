#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#if defined(_WIN32)
#define NESTED_DIVISION_EXPORT __declspec(dllexport)
#else
#define NESTED_DIVISION_EXPORT
#endif

struct word { int64_t value; };
struct division { struct word quotient; struct word remainder; };

_Static_assert(sizeof(long long) == 8, "lldiv operands are signed 64-bit values");
_Static_assert(sizeof(struct word) == 8, "Word contains one signed 64-bit value");
_Static_assert(offsetof(struct word, value) == 0, "Word.value starts at zero");
_Static_assert(_Alignof(struct word) == _Alignof(int64_t), "Word retains scalar alignment");
_Static_assert(sizeof(struct division) == 16, "Division contains two complete Words");
_Static_assert(offsetof(struct division, quotient) == 0, "quotient has its own first path");
_Static_assert(offsetof(struct division, remainder) == 8, "remainder has its own second path");
_Static_assert(_Alignof(struct division) == _Alignof(int64_t), "Division retains ordinary record alignment");

NESTED_DIVISION_EXPORT struct division nesteddiv_lldiv(int64_t numerator, int64_t denominator) {
    /* Refuse C's undefined input pairs before invoking the real libc operation. */
    if (denominator == 0 || (numerator == INT64_MIN && denominator == -1)) abort();
    lldiv_t value = lldiv((long long)numerator, (long long)denominator);
    struct division result = {{(int64_t)value.quot}, {(int64_t)value.rem}};
    return result;
}

NESTED_DIVISION_EXPORT bool nesteddiv_verify(struct division first, struct division copied) {
    return first.quotient.value == INT64_C(-23333333333)
        && first.remainder.value == -1
        && copied.quotient.value == INT64_C(-23333333333)
        && copied.remainder.value == -1;
}

#ifdef NESTED_DIVISION_FIXTURE_TEST
#include <assert.h>

int main(void) {
    struct division first = nesteddiv_lldiv(INT64_C(-70000000000), INT64_C(3));
    struct division copied = first;
    struct division second = nesteddiv_lldiv(INT64_C(70000000000), INT64_C(9));
    assert(first.quotient.value == INT64_C(-23333333333));
    assert(first.remainder.value == -1);
    assert(copied.quotient.value == INT64_C(-23333333333));
    assert(copied.remainder.value == -1);
    assert(second.quotient.value == INT64_C(7777777777));
    assert(second.remainder.value == 7);
    assert(nesteddiv_verify(first, copied));
    struct division wrong = copied;
    wrong.remainder.value = 0;
    assert(!nesteddiv_verify(first, wrong));
    return 0;
}
#endif
