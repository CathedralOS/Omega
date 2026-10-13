"""Freeze a global complex boundary solve, independent of matrix propagation.

Unknowns are r, forward/backward amplitudes at each layer's left face, and t
at the substrate face. Continuity of E and n*(forward-backward) supplies two
rows at each interface. Uses partial-pivot complex Gaussian elimination.
"""
import cmath
import json
import math
from pathlib import Path
ROOT = Path(__file__).resolve().parent


def solve(ambient, substrate, layers, wavelength):
    size = 2 * len(layers) + 2
    rows = []
    for boundary in range(len(layers) + 1):
        for magnetic in (False, True):
            row = [0j] * (size + 1)
            if boundary == 0:
                row[0] = -ambient if magnetic else 1
                row[-1] = -ambient if magnetic else -1
            else:
                index, thickness = layers[boundary - 1]
                phase = cmath.exp(2j * math.pi * index * thickness / wavelength)
                row[2 * boundary - 1] = phase * (index if magnetic else 1)
                row[2 * boundary] = (1 / phase) * (-index if magnetic else 1)
            if boundary == len(layers):
                row[-2] = -substrate if magnetic else -1
            else:
                index = layers[boundary][0]
                row[2 * boundary + 1] = -index if magnetic else -1
                row[2 * boundary + 2] = index if magnetic else -1
            rows.append(row)
    for column in range(size):
        pivot = max(range(column, size), key=lambda row: abs(rows[row][column]))
        rows[column], rows[pivot] = rows[pivot], rows[column]
        assert abs(rows[column][column]) > 1e-12
        divisor = rows[column][column]
        rows[column] = [value / divisor for value in rows[column]]
        for row in range(size):
            if row != column:
                factor = rows[row][column]
                rows[row] = [value - factor * chosen for value, chosen in zip(rows[row], rows[column])]
    reflection, transmission = rows[0][-1], rows[-1][-1]
    return [reflection.real, reflection.imag, transmission.real, transmission.imag,
            abs(reflection)**2, substrate / ambient * abs(transmission)**2]


def make(name, ambient, substrate, layers, wavelengths):
    tokens = [ambient, substrate, len(layers), len(wavelengths)]
    for layer in layers:
        tokens.extend(layer)
    tokens.extend(wavelengths)
    outputs = [solve(ambient, substrate, layers, wavelength) for wavelength in wavelengths]
    assert all(abs(row[-2] + row[-1] - 1) < 1e-12 for row in outputs)
    return dict(name=name, input=' '.join(map(str, tokens)) + '\n', ambient=ambient,
                substrate=substrate, layers=layers, wavelengths=wavelengths, outputs=outputs)


def main():
    cases = [make('bare', 1, 1.5, [], [550, 200, 2000]),
             make('quarter_wave', 1, 4, [[2, 75]], [400, 600, 1000]),
             make('zero_thickness', 1.2, 2.7, [[4, 0], [1, 0], [2.5, 0]], [550]),
             make('matched', 1.7, 1.7, [[1.7, 337.5], [1.7, 600]], [730]),
             make('forward', 1.2, 3.1, [[2.4, 125], [1.1, 340.75], [3.7, 87.5]], [200, 401.25, 550, 1000, 2000]),
             make('reverse', 3.1, 1.2, [[3.7, 87.5], [1.1, 340.75], [2.4, 125]], [200, 401.25, 550, 1000, 2000]),
             make('maximum', 4, 1, [[1 + (position % 4), 2000 - 211 * position] for position in range(8)], [200 + 50 * position for position in range(32)])]
    eof = make('eof', 1, 1.5, [], [550])
    eof['input'] = eof['input'].rstrip()
    twelve = make('twelve_digits', 1, 1.5, [], [550])
    twelve['input'] = '1.00000000000 1.5 0 1 550'
    budget = make('byte_frontier', 1, 1.5, [], [550])
    budget['input'] = budget['input'].rstrip().ljust(4095)
    cases.extend([eof, twelve, budget])
    # Analytic boundary cases challenge sign, field reference, and normalization.
    for name in ('bare', 'zero_thickness'):
        case = next(case for case in cases if case['name'] == name)
        ambient, substrate = case['ambient'], case['substrate']
        expected = [(ambient-substrate)/(ambient+substrate), 0, 2*ambient/(ambient+substrate), 0]
        assert all(abs(left-right) < 1e-12 for row in case['outputs'] for left,right in zip(row, expected))
    quarter = cases[1]['outputs'][1]
    assert abs(quarter[0]) < 1e-12 and abs(quarter[1]) < 1e-12
    assert abs(quarter[2]) < 1e-12 and abs(quarter[3] - 0.5) < 1e-12
    for forward, reverse in zip(cases[4]['outputs'], cases[5]['outputs']):
        assert abs(forward[-1] - reverse[-1]) < 1e-12
        assert abs(forward[-2] - reverse[-2]) < 1e-12
        assert abs(complex(*forward[2:4]) * 3.1 / 1.2 - complex(*reverse[2:4])) < 1e-12
    (ROOT/'fixtures.json').write_text(json.dumps(cases, indent=2) + '\n')
    (ROOT/'input.txt').write_text(cases[1]['input'])
    print(f'frozen: {len(cases)} cases, {sum(len(case["outputs"]) for case in cases)} wavelength rows; global boundary equations and analytic assertions')

if __name__ == '__main__':
    main()
