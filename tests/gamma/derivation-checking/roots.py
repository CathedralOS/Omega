"""The last checked conclusion must match both owner fields in orientation."""

from proof_wire import IDENTITY, NAT, ZERO, failure, proof_row, record, theory, vector


def cases():
    definitions = theory(NAT, (IDENTITY,))
    owners = (ZERO, record(2, 1, 1, 1))
    start = proof_row(definitions=definitions, owners=owners)
    proof = record(5, 2, 1, 1)
    yield vector("owner_root_orientation", failure(start + 8), (proof,), owners,
                 left=1, right=2, definitions=definitions)
    yield vector("owner_root_right", failure(start + 12), (proof,), owners,
                 left=2, right=2, definitions=definitions)
    yield vector("last_row_not_an_earlier_matching_conclusion",
                 failure(proof_row((proof,), definitions, owners) + 8),
                 (proof, record(1, 1, 1)), owners, left=2, right=1, definitions=definitions)
    yield vector("valid_prefix_cannot_hide_later_invalid_rule",
                 failure(proof_row((proof,), definitions, owners) + 12),
                 (proof, record(1, 2, 1)), owners, left=2, right=1, definitions=definitions)
