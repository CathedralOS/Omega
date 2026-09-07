"""Literal ground inputs and expected coordinates; no decoder or sort model."""

import struct

from wire import certificate, clause, envelope, example, function, proposition, record, theory


NAT = (record(1, 0), record(1, 1, 1))
ZERO = record(1, 1, 0)
NAT_THEORY = theory(NAT)


def failure(coordinate, code=8, tag=1, limit=0, requested=0):
    return bytes([tag]) + struct.pack("<4Q", code, coordinate, limit, requested)


def owner_row(definitions, preceding=()):
    # Outer header, complete authored theory, proposition magic/count, prefix rows.
    return 24 + len(definitions) + 8 + sum(map(len, preceding))


def root_left(definitions, owners):
    return owner_row(definitions, owners)


def witness_row(definitions, owners, preceding=()):
    # Eight root bytes finish the proposition; witness magic/count adds eight.
    return root_left(definitions, owners) + 16 + sum(map(len, preceding))


def grounded(sections, owner_count, witness_count, left, right, witness_bytes):
    first = 24 + len(sections[0])
    second = first + len(sections[1])
    third = second + len(sections[2])
    proof_table = second + 8 + witness_bytes
    return b"\x05" + struct.pack("<8Q", first, second, third, owner_count,
                                witness_count, left, right, proof_table)


def vector(name, owners=(ZERO,), witnesses=(), left=1, right=1,
           definitions=NAT_THEORY, proofs=(), expected=None, repetitions=2, timeout=60):
    sections = (definitions, proposition(owners, left, right), certificate(witnesses, proofs))
    observation = grounded(sections, len(owners), len(witnesses), left, right,
                           sum(map(len, witnesses)))
    return name, envelope(sections), observation if expected is None else expected, repetitions, timeout


def constant(result=1, constructor=1):
    return function((), (clause((record(1, constructor, 0),), body=1),), result=result)
