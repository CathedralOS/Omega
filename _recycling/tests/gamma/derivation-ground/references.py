"""Zero, self, forward, and cross-table references cannot acquire custody."""

from ground_wire import NAT, NAT_THEORY, ZERO, constant, failure, owner_row, record, theory, vector, witness_row


def cases():
    for reference in (0, 1, 2):
        yield vector(f"owner_first_row_child_{reference}", (record(1, 2, 1, reference),),
                     expected=failure(owner_row(NAT_THEORY) + 16))
    for reference in (0, 2, 3):
        yield vector(f"owner_later_unused_child_{reference}", (ZERO, record(1, 2, 1, reference)),
                     expected=failure(owner_row(NAT_THEORY, (ZERO,)) + 16))
    yield vector("owner_cycle_rejects_first_forward_edge", (
        record(1, 2, 1, 2), record(1, 2, 1, 1)), expected=failure(owner_row(NAT_THEORY) + 16))
    yield vector("witness_cannot_supply_missing_owner_child", (
        ZERO, record(1, 2, 1, 3)), (ZERO,),
        expected=failure(owner_row(NAT_THEORY, (ZERO,)) + 16))
    owners = (ZERO, record(1, 2, 1, 1))
    for reference in (0, 3, 4):
        yield vector(f"witness_first_child_{reference}", owners, (record(1, 2, 1, reference),),
                     expected=failure(witness_row(NAT_THEORY, owners) + 16))
    first = record(1, 2, 1, 2)
    for reference in (0, 4, 5):
        yield vector(f"witness_later_unused_child_{reference}", owners,
                     (first, record(1, 2, 1, reference)),
                     expected=failure(witness_row(NAT_THEORY, owners, (first,)) + 16))
    yield vector("witness_cycle_rejects_first_forward_edge", owners,
                 (record(1, 2, 1, 4), record(1, 2, 1, 3)),
                 expected=failure(witness_row(NAT_THEORY, owners) + 16))
    # A clause-local row1 does not create a ground row1 before its definition.
    definitions = theory(NAT, (constant(),))
    yield vector("template_row_is_not_an_owner_child", (record(1, 2, 1, 1),),
                 definitions=definitions, expected=failure(owner_row(definitions) + 16))
