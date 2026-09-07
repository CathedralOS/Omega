"""Literal templates, ground rows, and outcomes; no host substitution model."""

import struct

from wire import certificate, clause, envelope, example, function, proposition, record, theory

NAT = (record(1, 0), record(1, 1, 1))
ZERO = record(1, 1, 0)
IDENTITY = function((1,), (clause((record(0, 0),), body=1),))


def compared(equal, steps):
    return b"\x06" + struct.pack("<2Q", equal, steps)


def failure(coordinate, code=10, tag=1, limit=0, requested=0):
    return bytes([tag]) + struct.pack("<4Q", code, coordinate, limit, requested)


def vector(name, expected, owners=(ZERO, record(2, 1, 1, 1)), left=2, right=1,
           definitions=None, witnesses=(), proofs=(), entry="root", repetitions=2, timeout=60):
    if definitions is None:
        definitions = theory(NAT, (IDENTITY,))
    sections = (definitions, proposition(owners, left, right), certificate(witnesses, proofs))
    return name, entry, envelope(sections), expected, repetitions, timeout


def ordinary(rows, arguments=(), body=1):
    return function(arguments, (clause(rows, body=body),))
