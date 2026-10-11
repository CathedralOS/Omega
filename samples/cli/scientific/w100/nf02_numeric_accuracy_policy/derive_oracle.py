"""Independent rational/IEEE derivation; this does not execute Omega."""
from fractions import Fraction
import struct


def binary32(value):
    return struct.unpack('>f', struct.pack('>f', value))[0]


one = Fraction(1)
half_ulp32 = Fraction(1, 2**24)
midpoint32 = binary32(float(one + half_ulp32))
assert Fraction(midpoint32) == one
assert Fraction(float(one + half_ulp32)) == one + half_ulp32
assert Fraction(binary32(16777217)) == 16777216
assert Fraction(float(16777217)) == 16777217
addition_error64 = Fraction(0.1 + 0.2) - Fraction(0.3)
assert addition_error64 == Fraction(1, 2**54)
assert addition_error64 <= Fraction(1, 2**51)
assert Fraction(1.125) - one == Fraction(1, 8)
assert Fraction(1, 8) > Fraction(1, 16)
assert Fraction(1) - Fraction(-1) == 2
print('Independent oracle derivation (no Omega execution):')
print('binary32(1 + 2^-24) =', Fraction(midpoint32))
print('binary64(1 + 2^-24) =', Fraction(float(one + half_ulp32)))
print('binary32(2^24 + 1) =', Fraction(binary32(16777217)))
print('binary64 addition error =', addition_error64)
print('absolute dyadic difference =', Fraction(1.125) - one)
print('normalized opposite-extreme difference =', Fraction(1) - Fraction(-1))


def power_of_two(exponent):
    return Fraction(2**exponent) if exponent >= 0 else Fraction(1, 2**(-exponent))


def nearest_binary(value, precision, minimum_exponent, maximum_exponent):
    """Independent exact-rational rounding; None denotes positive infinity."""
    assert value >= 0
    if value == 0:
        return Fraction(0)
    exponent = value.numerator.bit_length() - value.denominator.bit_length()
    if value < power_of_two(exponent):
        exponent -= 1
    quantum = power_of_two(max(minimum_exponent - precision + 1,
                               exponent - precision + 1))
    units = value / quantum
    integral, remainder = divmod(units.numerator, units.denominator)
    twice_remainder = 2 * remainder
    if twice_remainder > units.denominator or (twice_remainder == units.denominator and integral % 2):
        integral += 1
    rounded = integral * quantum
    maximum = (2 - power_of_two(1 - precision)) * power_of_two(maximum_exponent)
    return None if rounded > maximum else rounded


print('Additive policy derivation (exact Fractions, no Omega execution):')
assert abs(Fraction(9, 8) - 1) == Fraction(1, 16) + Fraction(1, 16)
assert abs(Fraction(2) - 1) > Fraction(1, 2) * abs(Fraction(1))
assert abs(Fraction(1) - 2) <= Fraction(1, 2) * abs(Fraction(2))
for width, precision, minimum, maximum_exponent in [(32, 24, -126, 127), (64, 53, -1022, 1023)]:
    maximum = (2 - power_of_two(1 - precision)) * power_of_two(maximum_exponent)
    smallest = power_of_two(minimum - precision + 1)
    rounding = lambda value: nearest_binary(value, precision, minimum, maximum_exponent)
    assert rounding(2 * maximum) is None
    assert rounding(Fraction(3, 2) * maximum) is None
    assert rounding(maximum / 2) == maximum / 2
    assert rounding(maximum / 2 + maximum / 2) == maximum
    three_quarters_maximum = rounding(Fraction(3, 4) * maximum)
    assert three_quarters_maximum is not None and three_quarters_maximum < maximum
    assert rounding(smallest) == smallest
    assert rounding(2 * smallest) == 2 * smallest
    assert rounding(smallest / 2) == 0
    assert abs(smallest - 0) > 1 * abs(Fraction(0))
    assert abs(0 - smallest) <= 1 * abs(smallest)
    print(f'f{width}: RN(2M)=Inf; RN(1.5M)=Inf; RN(M/2 + M/2)=M; RN(3M/4)<M')
    print(f'f{width}: least subnormal=2^{minimum-precision+1}; RN(s/2)=0; s>0; s<=s')
