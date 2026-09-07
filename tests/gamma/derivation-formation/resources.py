"""Literal exact/adjacent provisions and one large local-template table."""

from theory_wire import NAT, failure, function, ordinary, record, theory, vector


def cases():
    yield vector("exact_sort_limit_reaches_inhabitation", (), sorts=65536,
                 expected=failure(28))
    yield vector("adjacent_sort_limit", (), sorts=65537,
                 expected=failure(28, 2, 2, 65536, 65537))
    yield vector("last_sort_mark_uses_full_tree_height", (record(65536, 0),), sorts=65536,
                 expected=failure(28))
    # E=(S+1)*(C+A)+4W+S. Constructor1 deliberately has invalid result0,
    # so the exact work bound continues to the independently anchored signature.
    exact = (record(0, 128, *([1] * 128)), record(1, 0))
    assert len(theory(exact, sorts=64030)) == 4 + 4 * 137
    assert 64031 * 130 + 4 * 137 + 64030 == 8388608
    yield vector("exact_work_limit_reaches_signature", exact, sorts=64030,
                 expected=failure(40))
    adjacent = (record(0, 148, *([1] * 148)),) + (record(1, 0),) * 9
    assert len(theory(adjacent, sorts=52753)) == 4 + 4 * 181
    assert 52754 * 158 + 4 * 181 + 52753 == 8388609
    yield vector("adjacent_work_limit_before_signature", adjacent, sorts=52753,
                 expected=failure(28, 3, 2, 8388608, 8388609))
    constructors = (record(1, 0),) * 65536
    assert 65537 * 65536 + 4 * 196611 + 65536 == 4295884812
    yield vector("work_diagnostic_exceeds_u32", constructors, sorts=65536,
                 expected=failure(28, 3, 2, 8388608, 4295884812), repetitions=1, timeout=600)
    # The function/constructor product alone is the limit. Its removal would
    # expose the invalid first constructor signature instead of this refusal.
    constructors = (record(0, 0),) + (record(1, 0),) * 2047
    definitions = (function((1,), result=0),) * 4096
    estimate = 2 * 2048 + 4096 * 2048 + 4 * (3 + 3 * 2048 + 7 * 4096) + 1
    assert len(theory(constructors, definitions)) == 4 + 4 * (3 + 3 * 2048 + 7 * 4096)
    assert estimate == 8531981
    yield vector("function_constructor_work_product", constructors, definitions,
                 expected=failure(28, 3, 2, 8388608, 8531981), repetitions=1, timeout=600)
    rows = (record(0, 0),) + tuple(record(1, 2, 1, previous) for previous in range(1, 46484))
    definitions = (ordinary(rows, (1,), 46484),)
    words = (len(theory(NAT, definitions)) - 4) // 4
    assert 2 * 3 + 2 + 4 * words + 1 < 8388608
    yield vector("46484_deep_local_template_rows", functions=definitions, repetitions=1, timeout=600)
