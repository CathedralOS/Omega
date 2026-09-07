"""Earlier physical, formation, and resource outcomes retain their fields."""

from ground_wire import (NAT, NAT_THEORY, ZERO, certificate, clause, envelope, failure,
                         function, owner_row, proposition, record, theory, vector)
from wire import changed_word


def cases():
    def direct(name, request, coordinate, code):
        return name, request, failure(coordinate, code), 2, 60

    yield direct("outer_short_header", b"", 0, 1)
    valid = envelope((NAT_THEORY, proposition((ZERO,), 1, 1), certificate()))
    yield direct("outer_identity", b"X" + valid[1:], 0, 2)
    yield direct("outer_trailing", valid + b"X", len(valid), 5)
    yield direct("outer_extent", valid[:-1], 16, 4)
    yield direct("layout_high_word_before_ground_semantics", changed_word(valid, 40, 0x80000000), 43, 6)
    yield vector("owner_variable_is_physical_failure", (record(0, 0),),
                 expected=failure(owner_row(NAT_THEORY) + 4, 6))
    proof = record(9, 0, 0)
    bad_owner = record(1, 99, 0)
    sections = (NAT_THEORY, proposition((bad_owner,), 1, 1), certificate(proofs=(proof,)))
    # Certificate magic + witness count + proof count + proof record length.
    proof_tag = 24 + len(sections[0]) + len(sections[1]) + 16
    yield direct("later_proof_layout_before_owner_semantics", envelope(sections), proof_tag, 6)
    yield vector("formation_signature_before_owner", (bad_owner,),
                 definitions=theory((record(0, 0),)), expected=failure(40, 7))
    yield vector("formation_inhabitation_before_root", (), definitions=theory((record(1, 1, 1),)),
                 expected=failure(28, 7))
    yield vector("forward_sort_capacity", definitions=theory(sorts=65537),
                 expected=failure(28, 2, 2, 65536, 65537))
    constructors = (record(0, 148, *([1] * 148)),) + (record(1, 0),) * 9
    yield vector("forward_work_capacity", definitions=theory(constructors, sorts=52753),
                 expected=failure(28, 3, 2, 8388608, 8388609))
    # The existing formation gate admits this exact theory and deliberately
    # leaves the later owner/witness/proof defects untouched. Ground owns the
    # first owner symbol, ahead of its arity/child, roots, and witness defects.
    identity = function((1,), (clause((record(0, 0),), body=1),))
    definitions = theory(NAT, (identity,))
    assert len(definitions) == 100
    yield vector("formation_boundary_previously_left_ground_unchecked",
                 (record(2, 999, 1, 0),), (record(1, 0, 1, 99),), 999, 0,
                 definitions=definitions, proofs=(record(2, 0, 99, 999),),
                 expected=failure(140))
