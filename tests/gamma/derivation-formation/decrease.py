"""Acyclic row storage alone cannot justify direct recursive definitions."""

from theory_wire import (NAT, ZERO, clause, clause_start, failure, function,
                         function_start, natural_cases, ordinary, record, row_start, vector)


def cases():
    first = clause((ZERO,), constructor=1, body=1)
    successor_start = clause_start(function_start(NAT), 1, (first,))
    rows = (record(0, 1), record(1, 2, 1, 1), record(2, 1, 1, 2))
    yield vector("reconstructed_parent_not_decrease", functions=(natural_cases(rows, 3),),
                 expected=failure(row_start(successor_start, rows[:2]) + 16))
    rows = (ZERO, record(2, 1, 1, 1))
    yield vector("constant_not_decrease", functions=(natural_cases(rows, 2),),
                 expected=failure(row_start(successor_start, rows[:1]) + 16))
    identity = ordinary((record(0, 0),), (1,))
    rows = (record(0, 1), record(2, 1, 1, 1), record(2, 2, 1, 2))
    function_offset = function_start(NAT, (identity,))
    yield vector("helper_computed_child_not_decrease", functions=(identity, natural_cases(rows, 3)),
                 expected=failure(row_start(clause_start(function_offset, 1, (first,)), rows[:2]) + 16))
    rows = (record(0, 1), record(0, 2), record(2, 1, 2, 1, 2))
    # Slot1 is an unchanged other parameter of the correct sort, not a child.
    yield vector("other_parameter_not_decrease", functions=(natural_cases(rows, 3, (1, 1)),),
                 expected=failure(row_start(clause_start(function_start(NAT), 2, (first,)), rows[:2]) + 16))
    constructors = (record(1, 0), record(2, 0), record(1, 1, 2))
    branch = clause((record(0, 1), record(2, 1, 1, 1)), constructor=3, body=2)
    offset = clause_start(function_start(constructors), 1, (first,))
    yield vector("immediate_child_wrong_sort_precedes_decrease", constructors,
                 (function((1,), (first, branch), mode=1),), sorts=2,
                 expected=failure(row_start(offset, (record(0, 1),)) + 16))
    parent = clause((record(0, 0), record(2, 1, 1, 1)), constructor=2, body=2)
    yield vector("matched_parent_unbound_before_self_call", functions=(
        function((1,), (first, parent), mode=1),), expected=failure(row_start(successor_start) + 8))
    # Other arguments are checked before the decrease relation is inspected.
    rows = (ZERO, record(2, 1, 2, 1, 0))
    yield vector("later_argument_failure_before_nondecrease", functions=(natural_cases(rows, 2, (1, 1)),),
                 expected=failure(row_start(clause_start(function_start(NAT), 2, (first,)), rows[:1]) + 20))
