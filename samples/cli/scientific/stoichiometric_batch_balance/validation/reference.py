#!/usr/bin/env python3
"""Independent arbitrary-precision oracle; sum-ordered compositions, no radix scan."""
from itertools import combinations
from math import gcd
from functools import reduce

MAXIMUM = 2147483647

def compositions(total, count):
    for cuts in combinations(range(1, total), count - 1):
        points = (0, *cuts, total)
        yield tuple(points[i+1] - points[i] for i in range(count))

def reference(raw):
    if len(raw) > 1024 or any(c not in b"0123456789 \t\r\n" for c in raw):
        return 64, "INVALID_INPUT\n"
    words = raw.split()
    if len(words) != 19:
        return 64, "INVALID_INPUT\n"
    values = list(map(int, words))
    if any(value > MAXIMUM for value in values):
        return 64, "INVALID_INPUT\n"
    count, bound = values[:2]
    atoms = [values[2+i*3:5+i*3] for i in range(4)]
    stock, cap = values[14:18], values[18]
    if count not in (3,4) or not 1 <= bound <= 8 or any(value > 16 for row in atoms for value in row):
        return 64, "INVALID_INPUT\n"
    if any(sum(atoms[i]) == 0 for i in range(3)):
        return 64, "INVALID_INPUT\n"
    if (count == 4 and sum(atoms[3]) == 0) or (count == 3 and (sum(atoms[3]) != 0 or stock[3] != 0)):
        return 64, "INVALID_INPUT\n"
    candidates = []
    for total in range(count, count*bound + 1):
        for active in compositions(total, count):
            if max(active) > bound or reduce(gcd, active) != 1:
                continue
            coefficients = active + (0,) * (4-count)
            if all(sum(coefficients[i]*atoms[i][element]*(1 if i<2 else -1) for i in range(4)) == 0 for element in range(3)):
                candidates.append(coefficients)
    if not candidates:
        return 65, "NO_BALANCE_WITHIN_BOUND\n"
    coefficients = min(candidates, key=lambda row: (sum(row), row))
    capacities = [stock[i]//coefficients[i] for i in range(2)]
    capacity = min(capacities)
    extent = min(capacity, cap)
    mask = sum(1<<i for i in range(2) if capacities[i] == capacity)
    inventory = [stock[i]+extent*coefficients[i]*(-1 if i<2 else 1) for i in range(4)]
    row = lambda name, values: name + " " + " ".join(map(str,values)) + "\n"
    if max(inventory) > MAXIMUM:
        return 66, "OVERFLOW_REFUSED\n" + row("inventory", stock)
    before = [sum(stock[i]*atoms[i][element] for i in range(4)) for element in range(3)]
    after = [sum(inventory[i]*atoms[i][element] for i in range(4)) for element in range(3)]
    assert before == after
    return 0, ("BALANCED\n" + row("coefficients", coefficients)
        + row("primitive_solutions", [len(candidates)]) + row("extent", [extent])
        + row("limiting_mask", [mask]) + row("cap_limited", [int(cap<capacity)])
        + row("inventory", inventory) + row("elements", before) + "CONSERVATION_OK\n")
