"""Exact reference/head/ordinal ordering, without resuming failed requests."""

from substitution_wire import NAT, ZERO, failure, ordinary, record, theory, vector


def cases():
    constant = theory(NAT, (ordinary((ZERO,)),))
    owners = (record(2, 1, 0), ZERO, ZERO)
    for name, left, right, coordinate in (
        ("equal_zero_ids", 1, 1, 901), ("equal_outside_ids", 2, 2, 901),
        ("left_before_right", 1, 2, 901), ("right_zero", 3, 1, 902),
        ("right_outside", 3, 2, 902),
    ):
        yield vector(name, failure(coordinate, 9), owners, left=left, right=right,
                     definitions=constant, entry="invalid")
    yield vector("right_reference_before_wrong_left_head", failure(902, 9), (ZERO, ZERO, ZERO),
                 left=3, right=1, definitions=constant, entry="invalid")
    yield vector("left_constructor_not_function", failure(901), (ZERO,), left=1,
                 definitions=constant)
    owners = (ZERO, record(2, 1, 1, 1), ZERO)
    yield vector("clause_zero", failure(903), owners, entry="clause")
    yield vector("clause_above_count", failure(903), owners, right=3, entry="clause")
    yield vector("invalid_clause_before_wrong_body", failure(903), owners, right=3, entry="case")
    yield "outer_failure_forwarded", "root", b"", failure(0, 1), 2, 60
    yield vector("formation_failure_forwarded", failure(40, 7),
                 definitions=theory((record(0, 0),)))
    # With the 100-byte identity theory, first owner row starts132, symbol140.
    yield vector("ground_failure_forwarded", failure(140, 8),
                 (record(2, 99, 0),), left=1)
    # Default owner rows are16+20 bytes; theoryend124/propositionend176;
    # proof tag is certificate start + magic/counts/record length16 =192.
    yield vector("proof_layout_precedes_unfolding", failure(192, 6), proofs=(record(9, 0, 0),))
