"""Authored record construction and coordinates, never a semantic decoder."""

import struct

from wire import certificate, clause, envelope, example, function, proposition, record, theory


NAT = (record(1, 0), record(1, 1, 1))
ZERO = record(1, 1, 0)


def formed(sections, sorts, constructors, functions):
    first = 24 + len(sections[0])
    second = first + len(sections[1])
    return b"\x04" + struct.pack("<6Q", first, second, second + len(sections[2]),
                                sorts, constructors, functions)


def failure(coordinate, code=7, tag=1, limit=0, requested=0):
    return bytes([tag]) + struct.pack("<4Q", code, coordinate, limit, requested)


def vector(name, constructors=NAT, functions=(), sorts=1, expected=None,
           ground=None, proofs=None, repetitions=2, timeout=60):
    sections = (theory(constructors, functions, sorts),
                proposition() if ground is None else ground,
                certificate() if proofs is None else proofs)
    observation = formed(sections, sorts, len(constructors), len(functions))
    return name, envelope(sections), observation if expected is None else expected, repetitions, timeout


def function_start(constructors, preceding=()):
    # Header24 + magic/sort/constructor count12 + encoded constructor rows +
    # function count4 + explicitly authored earlier function bytes.
    return 40 + sum(map(len, constructors)) + sum(map(len, preceding))


def clause_start(function_offset, arity, preceding=()):
    # Six words before the first clause: record length, result, arity,
    # mode, selected argument, and clause count; argument sorts add arity words.
    return function_offset + 24 + 4 * arity + sum(map(len, preceding))


def row_start(clause_offset, preceding=()):
    return clause_offset + 12 + sum(map(len, preceding))


def ordinary(rows, arguments=(), body=1, result=1):
    return function(arguments, (clause(rows, body=body),), result=result)


def natural_cases(successor_rows, successor_body=1, arguments=(1,), selected=0):
    return function(arguments, (
        clause((ZERO,), constructor=1, body=1),
        clause(successor_rows, constructor=2, body=successor_body),
    ), mode=1, selected=selected)
