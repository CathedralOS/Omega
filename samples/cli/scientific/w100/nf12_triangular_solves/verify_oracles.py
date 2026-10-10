#!/usr/bin/env python3
"""Check frozen hand-authored solutions against exact rational equations.

The oracle verifier does not call or reproduce the Omega substitution code.
Each expected X is certified by exact A*X=B, and each singular expectation
is certified by scanning the stored diagonal. Floating tolerances are frozen
in oracles.json and used by the Omega customers, not weakened by this script.
"""
from pathlib import Path
from fractions import Fraction
import hashlib
import json

path = Path(__file__).with_name('oracles.json')
raw = path.read_bytes()
record = json.loads(raw)
assert record['solution_absolute_tolerance'] == 1e-12
assert record['residual_absolute_tolerance'] == 1e-12
assert record['relative_tolerance'] == 0
order = record['order']
for fixture in record['fixtures']:
    matrix = [Fraction(value) for value in fixture['matrix']]
    rhs = [Fraction(value) for value in fixture['rhs']]
    if 'status' in fixture:
        pivots = [row for row in range(order) if matrix[row + row * order] == 0]
        assert pivots and pivots[0] == fixture['pivot'], fixture['name']
        assert fixture['status'] == 'zero_pivot' and fixture['unchanged']
        continue
    solution = [Fraction(value) for value in fixture['solution']]
    assert len(solution) == len(rhs) and len(rhs) % order == 0
    for column in range(len(rhs) // order):
        for row in range(order):
            result = Fraction(0)
            for inner in range(order):
                stored_row, stored_column = (inner, row) if fixture['transpose'] else (row, inner)
                in_triangle = stored_row >= stored_column if fixture['triangle'] == 'lower' else stored_row <= stored_column
                coefficient = matrix[stored_row + stored_column * order] if in_triangle else Fraction(0)
                if fixture['unit'] and row == inner:
                    coefficient = Fraction(1)
                result += coefficient * solution[inner + column * order]
            assert result == rhs[row + column * order], (fixture['name'], column, row, result)
print(f"certified {len(record['fixtures'])} frozen fixtures; sha256 {hashlib.sha256(raw).hexdigest()}")
