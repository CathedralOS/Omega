"""Independent exact-rational and high-precision checks of retained NF03 oracles.

Expected values and magnitude intervals come from the unchanged ORACLES.md.
This proves those mathematical fixtures, not compiler or runtime acceptance.
"""
from decimal import Decimal, localcontext
from fractions import Fraction
from pathlib import Path
import hashlib
import json
import struct


def product(left, right):
    return (left[0] * right[0] - left[1] * right[1],
            left[0] * right[1] + left[1] * right[0])


def main():
    voltage = (Fraction(3), Fraction(4))
    current = (Fraction(-2), Fraction(1, 2))
    conjugate = (current[0], -current[1])
    assert conjugate == (Fraction(-2), Fraction(-1, 2))
    assert product(voltage, conjugate) == (Fraction(-4), Fraction(-19, 2))
    assert tuple(first + second for first, second in zip(voltage, current)) == (Fraction(1), Fraction(9, 2))
    assert sum(component * component for component in voltage) == 25
    assert Fraction(7) ** 2 == 49
    point = (Fraction(1), Fraction(0))
    rotation = (Fraction(0), Fraction(1))
    expected = [(0, 1), (-1, 0), (0, -1), (1, 0)]
    points = []
    for answer in expected:
        point = product(point, rotation)
        assert point == answer
        assert sum(component * component for component in point) == 1
        points.append(point)
    assert tuple(sum(point[position] for point in points) for position in [0, 1]) == (0, 0)
    assert Fraction(3) ** 2 + Fraction(4) ** 2 == Fraction(5) ** 2
    # Decimal sqrt of the exact format-rounded components is independent of
    # the implementation's scaled ratio algorithm. Bounds stay frozen.
    with localcontext() as context:
        context.prec = 100
        for precision, real, imaginary, lower, upper in [
            (64, 3.0e200, 4.0e200, "4.99999999999999e200", "5.00000000000001e200"),
            (32, 3.0e20, 4.0e20, "4.999999e20", "5.000001e20"),
        ]:
            if precision == 32:
                real = struct.unpack("!f", struct.pack("!f", real))[0]
                imaginary = struct.unpack("!f", struct.pack("!f", imaginary))[0]
            real_decimal, imaginary_decimal = Decimal.from_float(real), Decimal.from_float(imaginary)
            magnitude = (real_decimal ** 2 + imaginary_decimal ** 2).sqrt()
            assert Decimal(lower) <= magnitude <= Decimal(upper)
    oracle = Path(__file__).with_name("ORACLES.md")
    print(json.dumps({"mathematical_oracles": "passed", "oracle_sha256": hashlib.sha256(oracle.read_bytes()).hexdigest(),
                      "qualification": "mathematics only; no runtime claim"}))


if __name__ == "__main__":
    main()
