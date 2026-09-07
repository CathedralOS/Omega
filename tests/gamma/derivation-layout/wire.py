"""Literal field encoders and diagnostic expectations; no wire decoder."""

import struct


MAGIC = b"GDREQ\x01\x00\x00"
LIMIT = 8388608


def words(*values):
    return struct.pack("<" + "I" * len(values), *values)


def record(*values):
    return words(len(values), *values)


def nested_record(payload):
    assert len(payload) % 4 == 0
    return words(len(payload) // 4) + payload


def table(records):
    return words(len(records)) + b"".join(records)


def theory(constructors=(), functions=(), sorts=1):
    return b"GTH1" + words(sorts) + table(constructors) + table(functions)


def proposition(terms=(), left=0, right=0):
    return b"GPR1" + table(terms) + words(left, right)


def certificate(terms=(), proofs=()):
    return b"GCE1" + table(terms) + table(proofs)


def clause(templates=(), constructor=0, body=0):
    return nested_record(words(constructor) + table(templates) + words(body))


def function(arguments=(), clauses=(), mode=0, selected=0, result=1):
    return nested_record(
        words(result, len(arguments), *arguments, mode, selected) + table(clauses)
    )


def envelope(sections):
    return MAGIC + words(*(len(section) for section in sections), 0) + b"".join(sections)


def layout(sections):
    first = 24 + len(sections[0])
    second = first + len(sections[1])
    return b"\x03" + words(first, second, second + len(sections[2]))


def rejected(coordinate, code=6):
    return b"\x01" + words(code, coordinate, 0, 0)


def changed_word(source, offset, value):
    return source[:offset] + words(value) + source[offset + 4:]


def example():
    # FORMAT's hand-worked identity(next(zero)) = next(zero) layout.
    sections = (
        theory((record(1, 0), record(1, 1, 1)),
               (function((1,), (clause((record(0, 0),), body=1),)),)),
        proposition((record(1, 1, 0), record(1, 2, 1, 1), record(2, 1, 1, 2)), 3, 2),
        certificate(proofs=(record(5, 3, 2, 1),)),
    )
    assert tuple(map(len, sections)) == (100, 72, 32)
    assert len(envelope(sections)) == 228
    return sections
