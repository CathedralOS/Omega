"""Application ordering, local backward references, and inferred sorts."""

from theory_wire import (NAT, ZERO, clause, clause_start, failure, function,
                         function_start, ordinary, record, row_start, vector)


def cases():
    offset = function_start(NAT)
    start = clause_start(offset, 0)
    first_row = row_start(start)
    for symbol in (0, 3):
        yield vector(f"constructor_symbol_{symbol}", functions=(ordinary((record(1, symbol, 0),)),),
                     expected=failure(first_row + 8))
    for symbol in (0, 2):
        yield vector(f"function_symbol_{symbol}", functions=(ordinary((record(2, symbol, 0),)),),
                     expected=failure(first_row + 8))
    yield vector("mode_zero_self_call", functions=(ordinary((record(2, 1, 0),)),),
                 expected=failure(first_row + 8))
    yield vector("existing_forward_function", functions=(
        ordinary((record(2, 2, 0),)), ordinary((ZERO,))), expected=failure(first_row + 8))
    for count, children in ((0, ()), (2, (0, 0))):
        yield vector(f"constructor_arity_{count}", functions=(ordinary((record(1, 2, count, *children),)),),
                     expected=failure(first_row + 12))
    for reference in (0, 1, 2):
        yield vector(f"first_row_child_{reference}", functions=(ordinary((record(1, 2, 1, reference),)),),
                     expected=failure(first_row + 16))
    for reference in (0, 2, 3):
        yield vector(f"unused_later_row_child_{reference}", functions=(
            ordinary((ZERO, record(1, 2, 1, reference))),),
            expected=failure(row_start(start, (ZERO,)) + 16))
    yield vector("symbol_before_arity_and_children", functions=(ordinary((record(1, 99, 1, 0),)),),
                 expected=failure(first_row + 8))
    yield vector("self_order_before_arity_and_children", functions=(ordinary((record(2, 1, 1, 0),)),),
                 expected=failure(first_row + 8))
    identity = ordinary((record(0, 0),), (1,))
    caller_start = clause_start(function_start(NAT, (identity,)), 0)
    yield vector("prior_function_wrong_arity", functions=(identity, ordinary((record(2, 1, 0),))),
                 expected=failure(row_start(caller_start) + 12))
    sorts = (record(1, 0), record(2, 0), record(1, 2, 1, 2))
    base = (ZERO, record(1, 2, 0))
    application_start = row_start(clause_start(function_start(sorts), 0), base)
    for name, children, child_index in (("first", (2, 1), 0), ("last", (1, 1), 1)):
        yield vector(f"constructor_argument_sort_{name}", sorts,
                     (ordinary(base + (record(1, 3, 2, *children),), body=3),), sorts=2,
                     expected=failure(application_start + 16 + 4 * child_index))
    typed_helper = ordinary((record(0, 0),), (1,))
    second_start = clause_start(function_start(sorts, (typed_helper,)), 0)
    yield vector("prior_function_argument_sort", sorts,
                 (typed_helper, ordinary(base + (record(2, 1, 1, 2),), body=3)), sorts=2,
                 expected=failure(row_start(second_start, base) + 16))
    earlier = clause((ZERO, record(1, 2, 1, 1), record(1, 2, 1, 2)), constructor=1, body=3)
    later = clause((ZERO, record(1, 2, 1, 3)), constructor=2, body=2)
    yield vector("template_reference_cannot_reach_previous_clause", functions=(
        function((1,), (earlier, later), mode=1),),
        expected=failure(row_start(clause_start(offset, 1, (earlier,)), (ZERO,)) + 16))
    unbound = record(0, 99)
    yield vector("unused_invalid_variable_before_valid_body", functions=(ordinary((ZERO, unbound)),),
                 expected=failure(row_start(start, (ZERO,)) + 8))
