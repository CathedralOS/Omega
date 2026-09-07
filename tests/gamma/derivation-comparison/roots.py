"""Structural syntax differs from definitional equality and proof validity."""

from comparison_wire import (NAT, ZERO, clause, compared, envelope, example,
                             function, record, theory, vector)


def cases():
    yield vector("same_identity", compared(1, 2))
    yield vector("distinct_equal_nullary", compared(1, 2), (ZERO, ZERO), right=2)
    yield vector("same_identity_composite_shortcut", compared(1, 2),
                 (ZERO, record(1, 2, 1, 1)), left=2, right=2)
    yield vector("constructor_head_mismatch", compared(0, 1),
                 (ZERO, record(1, 2, 1, 1)), right=2)
    nullaries = theory((record(1, 0), record(1, 0)))
    yield vector("different_nullary_symbols", compared(0, 1),
                 (ZERO, record(1, 2, 0)), right=2, definitions=nullaries)
    constant = function((), (clause((ZERO,), body=1),))
    yield vector("tag_precedes_same_symbol", compared(0, 1),
                 (ZERO, record(2, 1, 0)), right=2, definitions=theory(NAT, (constant,)))
    yield vector("different_functions_with_equal_bodies", compared(0, 1),
                 (record(2, 1, 0), record(2, 2, 0)), right=2,
                 definitions=theory(NAT, (constant, constant)))
    identity = function((1,), (clause((record(0, 0),), body=1),))
    yield vector("identity_application_does_not_reduce", compared(0, 1),
                 (ZERO, record(2, 1, 1, 1)), right=2, definitions=theory(NAT, (identity,)))
    sections = example()
    yield "format_unfolding_is_not_structural_equality", "root", envelope(sections), compared(0, 1), 2, 60
    branching = theory(NAT + (record(1, 0), record(1, 2, 1, 1)))
    rows = (ZERO, record(1, 3, 0), record(1, 4, 2, 1, 2), record(1, 4, 2, 2, 1))
    yield vector("ordered_children_first_mismatch", compared(0, 2), rows,
                 left=3, right=4, definitions=branching)
    rows = (ZERO, record(1, 3, 0), record(1, 4, 2, 1, 1), record(1, 4, 2, 1, 2))
    yield vector("same_head_false_suffix", compared(0, 4), rows,
                 left=3, right=4, definitions=branching)
    rows = (ZERO, ZERO, record(1, 4, 2, 1, 2), record(1, 4, 2, 2, 1))
    yield vector("ordered_equal_children", compared(1, 6), rows,
                 left=3, right=4, definitions=branching)
    rows = (ZERO, record(1, 2, 1, 1), ZERO, record(1, 2, 1, 3))
    yield vector("two_node_separate_encodings", compared(1, 4), rows, left=2, right=4)
    yield vector("proof_premises_still_unchecked", compared(1, 2), (ZERO, ZERO), right=2,
                 proofs=(record(2, 0, 99, 999),))
