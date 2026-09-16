"""Finite authored equations, grouped by their Beta encoding responsibility."""

import lexical
import nibbles
import roundtrip
import words
import counters
import ordering
import encoder


def cases(definitions):
    yield from lexical.cases(definitions)
    yield from nibbles.cases(definitions)
    yield from roundtrip.cases(definitions)
    yield from words.cases(definitions)
    yield from counters.cases(definitions)
    yield from ordering.cases(definitions)
    yield from encoder.cases(definitions)
