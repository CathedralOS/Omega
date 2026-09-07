"""Every rule and one connected derivation using all five rules."""

from proof_wire import (IDENTITY, NAT, ZERO, checked, envelope, example, record, theory, vector)


def cases():
    yield vector("reflexivity", checked(1, 9))
    yield vector("symmetry", checked(2, 15), (record(1, 1, 1), record(2, 1, 1, 1)))
    yield vector("transitivity_reuses_same_premise", checked(2, 17),
                 (record(1, 1, 1), record(3, 1, 1, 1, 1)))
    yield vector("zero_arity_congruence", checked(1, 7), (record(4, 1, 1, 0),))
    sections = example()
    yield "format_unfolding", envelope(sections), checked(1, 14), 2, 60
    owners = (ZERO, record(1, 2, 1, 1))
    yield vector("unary_congruence", checked(2, 16),
                 (record(1, 1, 1), record(4, 2, 2, 1, 1)), owners, left=2, right=2)
    definitions = theory(NAT + (record(1, 0), record(1, 2, 1, 1)))
    owners = (ZERO, record(1, 3, 0), record(1, 4, 2, 1, 2))
    yield vector("ordered_binary_congruence", checked(3, 25),
                 (record(1, 1, 1), record(1, 2, 2), record(4, 3, 3, 2, 1, 2)),
                 owners, left=3, right=3, definitions=definitions)
    yield vector("duplicate_structural_rows", checked(1, 9), (record(1, 1, 2),),
                 (ZERO, ZERO), left=1, right=2)
    yield vector("witness_aliases_conclude_owner", checked(1, 9), (record(1, 2, 3),),
                 witnesses=(ZERO, ZERO))
    owners = (ZERO, record(1, 2, 1, 1))
    witnesses = (ZERO, record(1, 2, 1, 3))
    yield vector("composite_witness_alias_root", checked(1, 11), (record(1, 4, 4),),
                 owners, witnesses, left=2, right=2)
    definitions = theory(NAT, (IDENTITY,))
    owners = (ZERO, record(2, 1, 1, 1), record(1, 2, 1, 2), record(1, 2, 1, 1))
    # Each row feeds the final congruence: unfold -> symmetry -> transitivity,
    # with reflexivity as transitivity's second premise.
    proofs = (record(5, 2, 1, 1), record(2, 1, 2, 1), record(1, 2, 2),
              record(3, 1, 2, 2, 3), record(4, 4, 3, 1, 4))
    yield vector("connected_all_five_rules", checked(5, 39), proofs, owners,
                 left=4, right=3, definitions=definitions)
