"""Freeze rational Simpson integration independently of Omega's coefficients.

The integrands have degree <=3, so Simpson's rule is exact on each clipped
linear-width span. Fraction arithmetic avoids copying floating-point mistakes.
This file generates fixtures/oracles; it never substitutes for the application.
"""
from fractions import Fraction as F
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    'rectangle': (10000, 0, [(0, 2000), (3000, 2000)], [0, 1000, 3000, 5000]),
    'triangle': (9810, 1000, [(0, 0), (4000, 4000)], [0, 2000, 4000, 5000]),
    'piecewise': (9810, 2000, [(0, 1000), (1000, 3000), (3000, 2000), (6000, 500)], [0, 500, 1000, 2250, 6000, 8000]),
    'elevated': (10000, 3000, [(2000, 1000), (4000, 2000)], [0, 2000, 3000, 4000, 5000]),
    'zero_width': (9810, 0, [(0, 0), (1000, 0), (2000, 2000)], [500, 1000, 1500, 3000]),
    'millimetre': (9810, 1, [(0, 1), (1, 2), (3, 0)], [0, 1, 2, 4]),
    'maximum': (20000, 100000, [(0, 100000), (100000, 100000)], [100000]),
    'capacity': (9810, 3000, [(0, 1000), (1000, 2000), (2000, 500), (3000, 3000), (4000, 1000), (5000, 0), (6000, 2000), (7000, 1000)], [0, 999, 1000, 2999, 3000, 6001, 7000, 9000]),
    'subdivided': (10000, 0, [(0, 2000), (1000, 2000), (2000, 2000), (3000, 2000)], [0, 1000, 3000, 5000]),
    'translated': (9810, 12000, [(10000, 1000), (11000, 3000), (13000, 2000), (16000, 500)], [10000, 10500, 11000, 12250, 16000, 18000]),
}
INVALID = {
    'empty': '',
    'count_small': '1 1 9810 0 0 1000 1000\n',
    'count_large': '9 1 9810 0\n',
    'no_levels': '2 0 9810 0\n',
    'gamma_zero': '2 1 0 0\n',
    'gamma_large': '2 1 20001 0\n',
    'hinge_large': '2 1 9810 100001\n',
    'duplicate': '2 1 9810 0 0 1000 0 1000 1000\n',
    'descending': '2 1 9810 0 1000 1000 0 1000 1000\n',
    'width_large': '2 1 9810 0 0 100001 1000 1000 1000\n',
    'water_large': '2 1 9810 0 0 1000 1000 1000 100001\n',
    'truncated': '2 1 9810 0 0 1000 1000\n',
    'negative': '2 1 9810 0 0 -1000 1000 1000 1000\n',
    'decimal': '2 1 9810 0 0 1.0 1000 1000 1000\n',
    'garbage': 'hello\n',
    'digit_overflow': '9999999999999999999\n',
    'extra_token': '2 1 9810 0 0 1000 1000 1000 1000 7\n',
    'extra_garbage': '2 1 9810 0 0 1000 1000 1000 1000 x\n',
    'too_long': ' ' * 4097,
}


def rounded(value):
    scaled = value * 1000000
    sign = -1 if scaled < 0 else 1
    magnitude = abs(scaled)
    return sign * ((2 * magnitude.numerator + magnitude.denominator) // (2 * magnitude.denominator))


def simpson(function, lower, upper):
    return (upper - lower) * (function(lower) + 4 * function((lower + upper) / 2) + function(upper)) / 6


def solve(gamma, hinge_mm, nodes_mm, levels_mm):
    nodes = [(F(z, 1000), F(width, 1000)) for z, width in nodes_mm]
    hinge = F(hinge_mm, 1000)
    results = []
    lines = ['HYDROSTATIC_GATE_V1 micro_units']
    for index, water_mm in enumerate(levels_mm):
        water = F(water_mm, 1000)
        lines.append(f'LEVEL {index} water_um={rounded(water)}')
        segments = []
        area = force = moment = first = F(0)
        for number, ((lower, width_lower), (upper, width_upper)) in enumerate(zip(nodes, nodes[1:])):
            wet_upper = min(upper, water)
            if wet_upper <= lower:
                continue
            def width(z):
                return width_lower + (z - lower) * (width_upper - width_lower) / (upper - lower)
            def pressure_width(z):
                return gamma * (water - z) * width(z)
            segment_area = simpson(width, lower, wet_upper)
            segment_force = simpson(pressure_width, lower, wet_upper)
            segment_first = simpson(lambda z: z * pressure_width(z), lower, wet_upper)
            segment_moment = simpson(lambda z: (z - hinge) * pressure_width(z), lower, wet_upper)
            row = dict(index=number, lower=lower, upper=wet_upper, lower_width=width(lower), upper_width=width(wet_upper), area=segment_area, force=segment_force, moment=segment_moment)
            segments.append(row)
            lines.append('WET ' + str(number) + ' ' + ' '.join(f'{key}={rounded(row[field])}' for key, field in [('lower_um','lower'), ('upper_um','upper'), ('lower_width_um','lower_width'), ('upper_width_um','upper_width'), ('area_micro_m2','area'), ('force_uN','force'), ('moment_uNm','moment')]))
            area += segment_area
            force += segment_force
            moment += segment_moment
            first += segment_first
        center = first / force if force else None
        lines.append(f'TOTAL wetted={len(segments)} area_micro_m2={rounded(area)} force_uN={rounded(force)} moment_uNm={rounded(moment)} center_um=' + (str(rounded(center)) if center is not None else 'undefined'))
        results.append(dict(water=water, segments=segments, area=area, force=force, moment=moment, center=center))
    return results, '\n'.join(lines) + '\n'


def freeze():
    # Independent analytic anchors before any Omega execution.
    rectangle, _ = solve(*CASES['rectangle'])
    assert rectangle[2]['area'] == 6
    assert rectangle[2]['force'] == 90000  # gamma*b*H^2/2
    assert rectangle[2]['moment'] == 90000  # gamma*b*H^3/6
    triangle, _ = solve(*CASES['triangle'])
    assert triangle[2]['force'] == F(9810 * 4**3, 6)
    assert triangle[2]['moment'] == F(9810 * 4**4, 12) - triangle[2]['force']
    subdivided, _ = solve(*CASES['subdivided'])
    for original, divided in zip(rectangle, subdivided):
        assert all(original[k] == divided[k] for k in ('area', 'force', 'moment', 'center'))
    original, _ = solve(*CASES['piecewise'])
    translated, _ = solve(*CASES['translated'])
    for left, right in zip(original, translated):
        assert all(left[k] == right[k] for k in ('area', 'force', 'moment'))
        assert right['center'] == left['center'] + 10 if left['center'] is not None else right['center'] is None
    data = ROOT / 'data'
    data.mkdir(exist_ok=True)
    expected = {}
    for name, (gamma, hinge, nodes, levels) in CASES.items():
        text = f'{len(nodes)} {len(levels)} {gamma} {hinge}\n' + ''.join(f'{z} {width}\n' for z, width in nodes) + ' '.join(map(str, levels)) + '\n'
        (data / f'{name}.in').write_text(text)
        results, output = solve(gamma, hinge, nodes, levels)
        expected[name] = results
        (data / f'{name}.expected.txt').write_text(output)
    for name, text in INVALID.items():
        (data / f'invalid_{name}.in').write_text(text)
    (data / 'invalid.expected.txt').write_text('ERROR invalid bounded gate input\n')
    # Serialize exact fractions as n/d for auditability.
    (data / 'oracle.json').write_text(json.dumps(expected, default=lambda v: str(v), indent=2) + '\n')
    # Tolerances are fixed independently, before check/native/Terminal execution.
    tolerances = {'absolute_micro_units': {'geometry': 1, 'area': 2, 'force': 2, 'moment': 4, 'center': 1}, 'relative': 2e-12, 'invalid_exit': 64, 'valid_exit': 0}
    (data / 'tolerances.json').write_text(json.dumps(tolerances, indent=2) + '\n')
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(data.iterdir()) if p.name != 'frozen.sha256.json'}
    (data / 'frozen.sha256.json').write_text(json.dumps(hashes, indent=2) + '\n')
    print(f'Frozen {len(CASES)} valid and {len(INVALID)} invalid input oracles; analytic and metamorphic anchors passed.')

if __name__ == '__main__':
    freeze()
