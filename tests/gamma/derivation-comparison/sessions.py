"""Exact cumulative steps expose memo keys and failed-parent custody."""

from comparison_wire import NAT, ZERO, compared, failure, record, theory, vector


def cases():
    chain = (ZERO, record(1, 2, 1, 1), record(1, 2, 1, 2),
             ZERO, record(1, 2, 1, 4), record(1, 2, 1, 5))
    yield vector("repeat_and_reverse_have_distinct_keys",
                 compared(1, 6) + compared(1, 8) + compared(1, 14), chain,
                 left=3, right=6, entry="session")
    yield vector("false_comparisons_keep_consumed_work",
                 compared(0, 1) + compared(0, 2) + compared(0, 3),
                 (ZERO, record(1, 2, 1, 1)), right=2, entry="session")
    definitions = theory(NAT + (record(1, 0), record(1, 2, 1, 1)))
    rows = (ZERO, ZERO, record(1, 2, 1, 1), record(1, 2, 1, 2), record(1, 3, 0),
            record(1, 4, 2, 3, 1), record(1, 4, 2, 4, 5))
    yield vector("completed_child_survives_but_false_parent_is_not_memoized",
                 compared(0, 6) + compared(1, 8) + compared(0, 12) + compared(1, 16),
                 rows, definitions=definitions, entry="retention")
    owners = (ZERO, record(1, 2, 1, 1))
    yield vector("cross_owner_witness_ordered_memo",
                 compared(1, 4) + compared(1, 6) + compared(1, 10), owners,
                 (ZERO, record(1, 2, 1, 3)), left=2, entry="witness")
    yield vector("cross_owner_witness_head_mismatch",
                 compared(0, 1) + compared(0, 2) + compared(0, 3), owners,
                 (ZERO,), left=2, entry="witness")
    yield vector("empty_witness_last_identity_remains_owner",
                 compared(1, 2) + compared(1, 4) + compared(1, 6), owners,
                 left=2, entry="witness")
    # Entry invalid maps checked roots1/2/3 to 0/N+1/1. Each invocation ends
    # after the one selected invalid pair; caller coordinates are literal701/709.
    for name, left, right, coordinate in (
        ("equal_zero_not_reflexive", 1, 1, 701),
        ("equal_outside_not_reflexive", 2, 2, 701),
        ("invalid_left_before_invalid_right", 1, 2, 701),
        ("outside_left_before_zero_right", 2, 1, 701),
        ("invalid_right_zero", 3, 1, 709),
        ("invalid_right_outside", 3, 2, 709),
    ):
        yield vector(name, failure(coordinate), (ZERO, ZERO, ZERO), left=left, right=right,
                     entry="invalid")
