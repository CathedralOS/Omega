"""Exact case coverage, clause-local slots, and body ownership."""

from theory_wire import (NAT, ZERO, clause, clause_start, failure, function,
                         function_start, ordinary, record, row_start, vector)


def cases():
    offset = function_start(NAT)
    constant = clause((ZERO,), body=1)
    yield vector("mode_zero_selected_must_be_zero", functions=(
        function((), (constant,), selected=1),), expected=failure(offset + 16))
    for count in (0, 2):
        yield vector(f"mode_zero_clause_count_{count}", functions=(
            function((), (constant,) * count),), expected=failure(offset + 20))
    yield vector("mode_zero_constructor_must_be_zero", functions=(
        function((), (clause((ZERO,), constructor=1, body=1),)),),
        expected=failure(clause_start(offset, 0) + 4))
    for arguments, selected in (((), 0), ((1,), 1), ((1,), 2)):
        yield vector(f"selected_outside_arity_{len(arguments)}_{selected}", functions=(
            function(arguments, (), mode=1, selected=selected),),
            expected=failure(offset + 16 + 4 * len(arguments)))
    first = clause((ZERO,), constructor=1, body=1)
    second = clause((record(0, 1),), constructor=2, body=1)
    start = clause_start(offset, 1)
    for name, definitions, coordinate in (
        ("missing_all_cases", (), offset + 24),
        ("missing_later_case", (first,), offset + 24),
        ("extra_case", (first, second, first), start + len(first) + len(second) + 4),
        ("duplicate_case", (first, first), start + len(first) + 4),
        ("reordered_case", (second, first), start + 4),
        ("zero_case_identity", (clause((ZERO,), constructor=0, body=1), second), start + 4),
        ("unknown_case_identity", (clause((ZERO,), constructor=99, body=1), second), start + 4),
    ):
        yield vector(name, functions=(function((1,), definitions, mode=1),), expected=failure(coordinate))
    wrong_sort = NAT + (record(2, 0),)
    wrong_first = clause((ZERO,), constructor=3, body=1)
    yield vector("case_of_wrong_sort", wrong_sort,
                 (function((1,), (wrong_first, second), mode=1),), sorts=2,
                 expected=failure(clause_start(function_start(wrong_sort), 1) + 4))
    for definitions in ((clause((), constructor=1),),
                        (clause((), constructor=1), second, first)):
        yield vector(f"earlier_empty_clause_before_coverage_{len(definitions)}", functions=(
            function((1,), definitions, mode=1),), expected=failure(start + 8))
    yield vector("empty_ordinary_template", functions=(function((), (clause(),)),),
                 expected=failure(clause_start(offset, 0) + 8))
    yield vector("unbound_ordinary_slot", functions=(ordinary((record(0, 1),), (1,)),),
                 expected=failure(row_start(start) + 8))
    parent = clause((record(0, 0),), constructor=1, body=1)
    yield vector("matched_parent_slot_is_unbound", functions=(
        function((1,), (parent, second), mode=1),), expected=failure(row_start(start) + 8))
    reversed_constructors = (record(1, 1, 1), record(1, 0))
    child = clause((record(0, 1),), constructor=1, body=1)
    leaked = clause((record(0, 1),), constructor=2, body=1)
    yield vector("child_slot_does_not_leak_to_next_clause", reversed_constructors,
                 (function((1,), (child, leaked), mode=1),),
                 expected=failure(row_start(start + len(child)) + 8))
    for body in (0, 2):
        authored = clause((ZERO,), body=body)
        yield vector(f"body_reference_{body}", functions=(function((), (authored,)),),
                     expected=failure(clause_start(offset, 0) + len(authored) - 4))
    two_sorts = (record(1, 0), record(2, 0))
    wrong_body = clause((record(1, 2, 0),), body=1)
    yield vector("body_result_sort", two_sorts, (function((), (wrong_body,)),), sorts=2,
                 expected=failure(clause_start(function_start(two_sorts), 0) + len(wrong_body) - 4))
