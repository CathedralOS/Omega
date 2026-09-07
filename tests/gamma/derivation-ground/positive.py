"""Sorted tables, unrestricted formed ground calls, and separate row custody."""

from ground_wire import (NAT, ZERO, clause, constant, envelope, example, function,
                         grounded, record, theory, vector)


def cases():
    sections = example()
    yield "format_identity", envelope(sections), grounded(sections, 3, 0, 3, 2, 0), 2, 60
    yield vector("one_owner_empty_witness_and_proof")
    yield vector("distinct_same_sort_roots_need_later_equality", (ZERO, record(1, 2, 1, 1)), right=2)
    yield vector("duplicate_owner_and_witness_rows", (ZERO, ZERO), (ZERO, ZERO), 1, 2)
    yield vector("unrelated_valid_rows_are_retained", (ZERO, record(1, 2, 1, 1)),
                 (record(1, 2, 1, 2), ZERO))
    functions = (constant(), constant(), constant())
    yield vector("highest_formed_function_is_first_ground_row", (record(2, 3, 0),),
                 definitions=theory(NAT, functions))
    recursive = function((1,), (
        clause((ZERO,), constructor=1, body=1),
        clause((record(0, 1), record(2, 1, 1, 1)), constructor=2, body=2),
    ), mode=1)
    yield vector("ground_calls_do_not_require_variable_decrease", (
        ZERO, record(2, 1, 1, 1), record(2, 1, 1, 2), record(1, 2, 1, 3)),
        (record(2, 1, 1, 4),), left=3, right=4, definitions=theory(NAT, (recursive,)))
    constructors = (record(1, 0), record(2, 0), record(1, 3, 1, 2, 1))
    functions = (constant(2, 2), function((2,), (clause((ZERO,), body=1),)))
    definitions = theory(constructors, functions, sorts=2)
    yield vector("tag_selected_namespaces_and_mixed_sorts", (
        record(2, 1, 0), ZERO, record(1, 3, 3, 2, 1, 2), record(2, 2, 1, 1)),
        (record(2, 1, 0), record(1, 3, 3, 2, 5, 4)), left=3, right=4,
        definitions=definitions)
    branching = theory(NAT + (record(1, 2, 1, 1),))
    yield vector("owner_and_witness_split_endpoints", (
        ZERO, record(1, 2, 1, 1), record(1, 2, 1, 2)), (
        record(1, 3, 2, 3, 1), record(1, 3, 2, 3, 4), record(1, 3, 2, 5, 2)),
        definitions=branching)
    yield vector("same_global_reference_retains_owner_sort", (ZERO, record(1, 2, 0)), (
        record(1, 2, 0), record(1, 3, 3, 1, 3, 1)), definitions=definitions)
    yield vector("repeated_children_and_duplicate_structure", (ZERO,
        record(1, 3, 2, 1, 1), record(1, 3, 2, 1, 1)), (
        record(1, 3, 2, 2, 3), record(1, 3, 2, 4, 4)), definitions=branching)
    invalid_proofs = (record(1, 0, 999), record(2, 1, 2, 999),
                      record(3, 0, 0, 999, 0), record(4, 1, 2, 0), record(5, 1, 2, 999))
    yield vector("all_proof_semantics_remain_later", (ZERO, record(1, 2, 1, 1)),
                 right=2, proofs=invalid_proofs)
