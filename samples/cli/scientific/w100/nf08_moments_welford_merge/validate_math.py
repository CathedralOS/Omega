"""Independent frozen rational oracles and floating algorithm experiments.

This does not execute, parse, or certify the Omega implementation.
"""
from fractions import Fraction
from itertools import permutations
from math import isfinite


def exact(values):
    count = len(values)
    mean = sum(map(Fraction, values)) / count
    central = sum((Fraction(value) - mean) ** 2 for value in values)
    return count, mean, central, central / count, central / (count - 1) if count > 1 else None


def stream(values):
    count, mean, central = 0, 0.0, 0.0
    for value in values:
        count += 1
        delta = value - mean
        mean += delta / count
        central += delta * (value - mean)
    return count, mean, central


def merge(left, right):
    if right[0] == 0:
        return left
    if left[0] == 0:
        return right
    count = left[0] + right[0]
    delta = right[1] - left[1]
    weight = right[0] / count
    return count, left[1] + delta * weight, left[2] + right[2] + delta * (delta * weight) * left[0]


def verify(summary, oracle, tolerance):
    count, mean, central = summary
    assert count == oracle[0]
    for actual, expected in [(mean, oracle[1]), (central, oracle[2]), (central / count, oracle[3])]:
        assert isfinite(actual) and abs(actual - float(expected)) <= tolerance, (actual, expected)
    if count > 1:
        assert abs(central / (count - 1) - float(oracle[4])) <= tolerance


observations = [2, 4, 4, 4, 5, 5, 7, 9]
oracle = exact(observations)
assert oracle == (8, Fraction(5), Fraction(32), Fraction(4), Fraction(32, 7))
verify(stream(observations), oracle, 1e-12)
assert exact([42]) == (1, Fraction(42), Fraction(0), Fraction(0), None)
assert stream([42]) == (1, 42.0, 0.0)

chunks = [[1000000001, 1000000002], [1000000003], [1000000004, 1000000005]]
values = [value for chunk in chunks for value in chunk]
oracle = exact(values)
assert oracle == (5, Fraction(1000000003), Fraction(10), Fraction(2), Fraction(5, 2))
verify(stream(values), oracle, 1e-6)
empty = (0, 0.0, 0.0)
for order in permutations(chunks):
    summaries = list(map(stream, order))
    verify(merge(merge(summaries[0], summaries[1]), summaries[2]), oracle, 1e-6)
    verify(merge(summaries[0], merge(summaries[1], summaries[2])), oracle, 1e-6)
    for summary in summaries:
        assert merge(empty, summary) == summary
        assert merge(summary, empty) == summary
# Vary split geometry independently of the fixed 2/1/2 sample.
for first_split in range(1, len(values)):
    verify(merge(stream(values[:first_split]), stream(values[first_split:])), oracle, 1e-6)
# The raw second-moment subtraction is not the frozen oracle.
raw_population = sum(float(value) ** 2 for value in values) / len(values) - (sum(values) / len(values)) ** 2
assert raw_population != 2.0
print('Exact rational fixtures verified; streaming and 12 merge trees plus 4 two-chunk splits satisfy frozen tolerances.')
print(f'Cancellation control raw E[x^2]-E[x]^2 = {raw_population}; exact population variance = 2.')
