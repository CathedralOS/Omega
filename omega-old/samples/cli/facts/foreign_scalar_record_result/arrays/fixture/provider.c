#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#if defined(_WIN32)
#define ARRAY_DIVISION_EXPORT __declspec(dllexport)
#else
#define ARRAY_DIVISION_EXPORT
#endif

struct word { int64_t value; uint8_t guard[4]; };
struct division { struct word parts[2]; };

_Static_assert(sizeof(long long) == 8, "lldiv operands are signed 64-bit values");
_Static_assert(sizeof(struct word) == 16, "Word retains its scalar, bytes and tail padding");
_Static_assert(offsetof(struct word, value) == 0, "Word.value starts at zero");
_Static_assert(offsetof(struct word, guard) == 8, "Word.guard follows the signed scalar");
_Static_assert(sizeof(((struct word *)0)->guard) == 4, "Word.guard has four byte elements");
_Static_assert(_Alignof(struct word) == _Alignof(int64_t), "Word retains scalar alignment");
_Static_assert(sizeof(struct division) == 32, "Division contains two complete Words");
_Static_assert(offsetof(struct division, parts) == 0, "parts starts at zero");
_Static_assert(offsetof(struct division, parts[1]) == 16, "the second Word retains its element stride");
_Static_assert(sizeof(((struct division *)0)->parts) == 32, "parts has two Word elements");
_Static_assert(_Alignof(struct division) == _Alignof(int64_t), "Division retains ordinary record alignment");

static const uint8_t earlier_guards[2][4] = {{17, 34, 51, 68}, {85, 102, 119, 136}};

ARRAY_DIVISION_EXPORT struct division arraydiv_lldiv(int64_t numerator, int64_t denominator) {
    if (denominator == 0 || (numerator == INT64_MIN && denominator == -1)) abort();
    lldiv_t value = lldiv((long long)numerator, (long long)denominator);
    struct division result = {0};
    result.parts[0].value = (int64_t)value.quot;
    result.parts[1].value = (int64_t)value.rem;
    uint8_t increment = numerator > 0 ? 1 : 0;
    for (size_t word = 0; word < 2; word++) {
        for (size_t byte = 0; byte < 4; byte++) {
            result.parts[word].guard[byte] = (uint8_t)(earlier_guards[word][byte] + increment);
        }
    }
    return result;
}

ARRAY_DIVISION_EXPORT bool arraydiv_verify(struct division first, struct division copied) {
    if (first.parts[0].value != INT64_C(-23333333333) || first.parts[1].value != -1
        || copied.parts[0].value != INT64_C(-23333333333) || copied.parts[1].value != -1) return false;
    for (size_t word = 0; word < 2; word++) {
        for (size_t byte = 0; byte < 4; byte++) {
            if (first.parts[word].guard[byte] != earlier_guards[word][byte]
                || copied.parts[word].guard[byte] != earlier_guards[word][byte]) return false;
        }
    }
    return true;
}

#ifdef ARRAY_DIVISION_FIXTURE_TEST
#include <assert.h>

int main(void) {
    struct division first = arraydiv_lldiv(INT64_C(-70000000000), INT64_C(3));
    struct division copied = first;
    struct division second = arraydiv_lldiv(INT64_C(70000000000), INT64_C(9));
    assert(first.parts[0].value == INT64_C(-23333333333));
    assert(first.parts[1].value == -1);
    assert(copied.parts[0].value == INT64_C(-23333333333));
    assert(copied.parts[1].value == -1);
    assert(second.parts[0].value == INT64_C(7777777777));
    assert(second.parts[1].value == 7);
    assert(arraydiv_verify(first, copied));
    for (size_t word = 0; word < 2; word++) {
        for (size_t byte = 0; byte < 4; byte++) {
            assert(first.parts[word].guard[byte] == earlier_guards[word][byte]);
            assert(copied.parts[word].guard[byte] == earlier_guards[word][byte]);
            assert(second.parts[word].guard[byte] == earlier_guards[word][byte] + 1);
        }
    }
    struct division wrong_word = first;
    wrong_word.parts[0].value++;
    assert(!arraydiv_verify(wrong_word, copied));
    struct division wrong_guard = copied;
    wrong_guard.parts[1].guard[2] ^= 1;
    assert(!arraydiv_verify(first, wrong_guard));
    return 0;
}
#endif
