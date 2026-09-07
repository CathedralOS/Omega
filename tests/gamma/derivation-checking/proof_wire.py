"""Authored wire records and field coordinates, never a host proof checker."""

import struct

from wire import certificate, clause, envelope, example, function, proposition, record, theory

NAT = (record(1, 0), record(1, 1, 1))
ZERO = record(1, 1, 0)
NAT_THEORY = theory(NAT)
IDENTITY = function((1,), (clause((record(0, 0),), body=1),))


def checked(count, steps):
    return b"\x07" + struct.pack("<2Q", count, steps)


def failure(coordinate, code=12, tag=1, limit=0, requested=0):
    return bytes([tag]) + struct.pack("<4Q", code, coordinate, limit, requested)


def proof_count(definitions=NAT_THEORY, owners=(ZERO,), witnesses=()):
    return 24 + len(definitions) + len(proposition(owners)) + 8 + sum(map(len, witnesses))


def proof_row(preceding=(), definitions=NAT_THEORY, owners=(ZERO,), witnesses=()):
    return proof_count(definitions, owners, witnesses) + 4 + sum(map(len, preceding))


def vector(name, expected, proofs=(record(1, 1, 1),), owners=(ZERO,), witnesses=(),
           left=1, right=1, definitions=NAT_THEORY, repetitions=2, timeout=60):
    sections = (definitions, proposition(owners, left, right), certificate(witnesses, proofs))
    return name, envelope(sections), expected, repetitions, timeout
