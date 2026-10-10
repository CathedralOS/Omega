"""Verify the frozen oracle identities with exact rational arithmetic.

This verifies authored fixtures, not an implementation of Cholesky or solving.
Run before Omega. Inputs and tolerances were frozen in commit 276f4aa16b.
"""
from fractions import Fraction
from pathlib import Path
import json

root = Path(__file__).resolve().parent
fixture_document = json.loads((root / "oracles.json").read_text())
for fixture in fixture_document["fixtures"][:2]:
    order = fixture["rows"]
    matrix = [Fraction(value) for value in fixture["a"]]
    factor = [Fraction(value) for value in fixture["l"]]
    solution = [Fraction(value) for value in fixture["x"]]
    rhs = [Fraction(value) for value in fixture["b"]]
    forward = [Fraction(value) for value in fixture["y"]]
    for row in range(order):
        assert sum(matrix[row + column * order] * solution[column]
                   for column in range(order)) == rhs[row]
        assert sum(factor[column + row * order] * solution[column]
                   for column in range(order)) == forward[row]
        assert sum(factor[row + column * order] * forward[column]
                   for column in range(order)) == rhs[row]
        assert factor[row + row * order] > 0
        for column in range(order):
            reconstructed = sum(factor[row + index * order]
                                * factor[column + index * order]
                                for index in range(order))
            assert reconstructed == matrix[row + column * order]
            if row < column:
                assert factor[row + column * order] == 0
    print(f"{fixture['name']}: exact A=L*L^T, L*y=b, L^T*x=y, A*x=b")
for fixture in fixture_document["fixtures"][2:]:
    if fixture["name"] == "nonsymmetric":
        assert fixture["a"][1] != fixture["a"][2]
    if fixture["name"] in ("indefinite", "semidefinite"):
        assert fixture["a"][0] == 1
        pivot = fixture["a"][3] - fixture["a"][1]**2
        assert pivot == (-3 if fixture["name"] == "indefinite" else 0)
    if fixture["name"] == "nonsquare":
        assert fixture["rows"] != fixture["columns"]
print("data-error oracle identities verified; no Omega result inferred")
