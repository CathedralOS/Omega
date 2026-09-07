"""Finite unsigned comparison diagnostics grouped by the compared vocabulary."""

from . import bytes, nibbles, words


def cases(definitions):
    yield from nibbles.cases(definitions)
    yield from bytes.cases(definitions)
    yield from words.cases(definitions)
