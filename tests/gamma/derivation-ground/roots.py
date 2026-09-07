"""Owner rows precede owner roots, and roots precede witness checking."""

from ground_wire import NAT_THEORY, ZERO, failure, owner_row, record, root_left, theory, vector


def cases():
    owners = (ZERO, record(1, 2, 1, 1))
    left_field = root_left(NAT_THEORY, owners)
    for reference in (0, 3, 99):
        yield vector(f"left_root_{reference}", owners, left=reference,
                     expected=failure(left_field))
        yield vector(f"right_root_{reference}", owners, right=reference,
                     expected=failure(left_field + 4))
    yield vector("left_root_before_right_root", owners, left=0, right=99,
                 expected=failure(left_field))
    yield vector("witness_cannot_rescue_left_root", owners, (ZERO,), left=3,
                 expected=failure(left_field))
    yield vector("witness_cannot_rescue_right_root", owners, (ZERO,), right=3,
                 expected=failure(left_field + 4))
    yield vector("empty_owner_rejects_left_field", (), (ZERO,), left=1, right=1,
                 expected=failure(root_left(NAT_THEORY, ())))
    definitions = theory((record(1, 0), record(2, 0)), sorts=2)
    different = (ZERO, record(1, 2, 0))
    yield vector("root_sorts_disagree_at_right", different, left=1, right=2, definitions=definitions,
                 expected=failure(root_left(definitions, different) + 4))
    yield vector("right_reference_before_sort_relation", different, left=2, right=3,
                 definitions=definitions, expected=failure(root_left(definitions, different) + 4))
    invalid_owner = (ZERO, record(1, 99, 0))
    yield vector("unused_owner_before_invalid_roots", invalid_owner, left=0, right=0,
                 expected=failure(owner_row(NAT_THEORY, (ZERO,)) + 8))
    invalid_witness = (record(1, 99, 0),)
    yield vector("owner_before_witness", invalid_owner, invalid_witness,
                 expected=failure(owner_row(NAT_THEORY, (ZERO,)) + 8))
    yield vector("root_before_invalid_witness", owners, invalid_witness, left=0,
                 expected=failure(left_field))
    yield vector("root_sort_before_invalid_witness", different, invalid_witness,
                 left=1, right=2, definitions=definitions,
                 expected=failure(root_left(definitions, different) + 4))
