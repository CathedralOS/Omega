"""Literal wire construction and authored observations; no equality algorithm."""

import struct

from wire import certificate, clause, envelope, example, function, proposition, record, theory


NAT = (record(1, 0), record(1, 1, 1))
ZERO = record(1, 1, 0)
NAT_THEORY = theory(NAT)


def compared(equal, steps):
    return b"\x06" + struct.pack("<2Q", equal, steps)


def failure(coordinate, code=9, tag=1, limit=0, requested=0):
    return bytes([tag]) + struct.pack("<4Q", code, coordinate, limit, requested)


def vector(name, expected, owners=(ZERO,), witnesses=(), left=1, right=1,
           definitions=NAT_THEORY, proofs=(), entry="root", repetitions=2, timeout=60):
    sections = (definitions, proposition(owners, left, right), certificate(witnesses, proofs))
    return name, entry, envelope(sections), expected, repetitions, timeout
