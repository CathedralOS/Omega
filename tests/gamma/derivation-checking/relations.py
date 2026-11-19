"""Each rule's structural requirements own distinct authored fields."""

from proof_wire import NAT, ZERO, failure, proof_row, record, theory, vector


def cases():
    definitions = theory((record(1, 0), record(1, 0)))
    owners = (ZERO, record(1, 2, 0))
    first = record(1, 1, 1)
    second = proof_row((first,), definitions, owners)
    yield vector("reflexivity_false", failure(proof_row(definitions=definitions, owners=owners) + 12),
                 (record(1, 1, 2),), owners, definitions=definitions)
    for name, row, coordinate in (
        ("symmetry_left", record(2, 2, 1, 1), second + 8),
        ("symmetry_right", record(2, 1, 2, 1), second + 12),
        ("transitivity_left", record(3, 2, 1, 1, 1), second + 8),
        ("transitivity_right", record(3, 1, 2, 1, 1), second + 12),
    ):
        yield vector(name, failure(coordinate), (first, row), owners, definitions=definitions)
    other = record(1, 2, 2)
    third = proof_row((first, other), definitions, owners)
    yield vector("transitivity_middle", failure(third + 20),
                 (first, other, record(3, 1, 2, 1, 2)), owners, definitions=definitions)
    yield vector("transitivity_second_reference_before_bad_left", failure(third + 20),
                 (first, other, record(3, 2, 2, 1, 0)), owners, definitions=definitions)
    branching = theory(NAT + (record(1, 0), record(1, 2, 1, 1)))
    owners = (ZERO, record(1, 3, 0), record(1, 4, 2, 1, 2))
    prefix = (record(1, 1, 1), record(1, 2, 2))
    third = proof_row(prefix, branching, owners)
    yield vector("congruence_symbol", failure(third + 12),
                 prefix + (record(4, 1, 2, 0),), owners, definitions=branching)
    for count in (0, 1, 3):
        yield vector(f"congruence_count_{count}", failure(third + 16),
                     prefix + (record(4, 3, 3, count, *([99] * count)),), owners, definitions=branching)
    for value in (0, 3, 4):
        yield vector(f"congruence_first_premise_{value}", failure(third + 20),
                     prefix + (record(4, 3, 3, 2, value, 2),), owners, definitions=branching)
        yield vector(f"congruence_second_premise_{value}", failure(third + 24),
                     prefix + (record(4, 3, 3, 2, 1, value),), owners, definitions=branching)
    yield vector("ordered_congruence_premises", failure(third + 20),
                 prefix + (record(4, 3, 3, 2, 2, 1),), owners, definitions=branching)
    yield vector("earlier_argument_before_later_invalid_premise", failure(third + 20),
                 prefix + (record(4, 3, 3, 2, 2, 99),), owners, definitions=branching)
