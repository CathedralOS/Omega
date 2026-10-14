#!/usr/bin/env python3
"""Independent cmath reference and strict observation checker; no Omega algorithm reuse."""
import argparse, cmath, hashlib, json, math, re
from pathlib import Path
ROOT = Path(__file__).resolve().parent
ORACLE_SHA256 = '3e352196111cd4296a2d556e5e896abc3ad7c7d55df9b233a14a8bafdf9a9699'

def reference(selection):
    positions = [0.0, 0.5, 1.0, 1.5]
    weights = [1.0] * 4
    steering = 0.0
    if selection == 1: steering = 0.5
    if selection == 2: weights = [0.0] * 4
    if selection == 3: positions = [0.0, 0.0, 0.5, 0.5]
    if selection == 4: weights = [1.0, 2.0, 2.0, 1.0]
    rows = []
    for index in range(9):
        direction = -1.0 + index * 0.25
        voltage = sum(weight * cmath.exp(2j * math.pi * position * (direction - steering))
                      for position, weight in zip(positions, weights))
        power = abs(voltage) ** 2
        denominator = sum(abs(weight) for weight in weights) ** 2
        rows.append(dict(index=index, u=direction, power=power,
                         normalized=power / denominator if denominator else 0.0,
                         null=int(power < 1e-10)))
    return dict(selection=selection, positions=positions, weights=weights, steering=steering,
                duplicates=sum(positions[left] == positions[right] for left in range(4) for right in range(left + 1, 4)),
                symmetry=max(abs(rows[index]['power'] - rows[8-index]['power']) for index in range(4)), samples=rows)

def checked_oracle():
    payload = (ROOT / 'data/oracle.json').read_bytes()
    assert hashlib.sha256(payload).hexdigest() == ORACLE_SHA256, 'Frozen oracle identity changed'
    oracle = json.loads(payload)
    assert len(oracle['cases']) == 5
    # cmath/libm may differ by last-bit rounding across supported hosts. The
    # checksum pins every authored value and tolerance; recomputation checks
    # the model within those unchanged tolerances instead of requiring one
    # host's floating-point representation. Discrete observations remain exact.
    for selection, case in enumerate(oracle['cases']):
        expected = reference(selection)
        for key in ('selection', 'positions', 'weights', 'steering', 'duplicates'):
            assert case[key] == expected[key], (selection, key)
        assert len(case['samples']) == len(expected['samples']) == 9
        assert abs(case['symmetry'] - expected['symmetry']) <= oracle['absolute_power_tolerance']
        for frozen, observed in zip(case['samples'], expected['samples']):
            for key in ('index', 'u', 'null'):
                assert frozen[key] == observed[key], (selection, key)
            for key, tolerance in (('power', 'absolute_power_tolerance'), ('normalized', 'absolute_normalized_tolerance')):
                assert abs(frozen[key] - observed[key]) <= oracle[tolerance], (selection, key)
    return oracle


def check(path, selection):
    oracle = checked_oracle()
    lines = Path(path).read_text().splitlines()
    assert lines.pop(0) == 'antenna_array_pattern scalar far-field u=-1:0.25:1; powers in millionths'
    selections = range(5) if selection is None else [selection]
    for selected in selections:
        case = oracle['cases'][selected]
        assert lines.pop(0) == f"case {selected} duplicates {case['duplicates']}"
        for row in case['samples']:
            match = re.fullmatch(r'sample (\d) power (\d{10}) normalized (\d{10}) null ([01])', lines.pop(0))
            assert match, f'Malformed row: {row}'
            index, power, normalized, null = map(int, match.groups())
            assert index == row['index'] and null == row['null'], (selected, row, match.groups())
            for observed, expected in [(power, row['power']), (normalized, row['normalized'])]:
                assert abs(observed - expected * 1e6) <= oracle['printed_micro_tolerance'], (selected, index, observed, expected)
        match = re.fullmatch(r'symmetry (\d{10})', lines.pop(0))
        assert match and abs(int(match[1]) - case['symmetry'] * 1e6) <= oracle['printed_micro_tolerance']
    assert lines == ['antenna_array_pattern complete'], lines
    print(f'PASS {len(list(selections))} cases; {9 * len(list(selections))} power/normalized/null rows; symmetry and duplicates')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('output', nargs='?')
    parser.add_argument('--selection', type=int, choices=range(5))
    options = parser.parse_args()
    if options.output: check(options.output, options.selection)
    else:
        checked_oracle()
        print('PASS frozen independent oracle: 45 direct complex sums')
