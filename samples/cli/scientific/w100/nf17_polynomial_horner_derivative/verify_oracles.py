#!/usr/bin/env python3
"""Independent rational power-sum oracle; no Horner recurrence."""
from fractions import Fraction
from pathlib import Path
import json
fixtures = json.loads(Path(__file__).with_name("oracles.json").read_text())
for name in ("cubic", "quadratic"):
    fixture = fixtures[name]
    coefficients = list(map(Fraction, fixture["coefficients"]))
    for query, value, derivative in zip(fixture["queries"], fixture["values"], fixture["derivatives"]):
        query = Fraction(query)
        assert sum(coefficient * query ** power for power, coefficient in enumerate(coefficients)) == Fraction(value)
        assert sum(power * coefficients[power] * query ** (power - 1) for power in range(1, len(coefficients))) == Fraction(derivative)
assert fixtures["constant"]["value"] == 7 and fixtures["constant"]["derivative"] == 0
print("10 independent exact rational fixtures verified")
