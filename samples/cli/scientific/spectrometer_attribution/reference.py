#!/usr/bin/env python3
"""Independent exact-rational oracle; never invokes or reads Omega output.

The oracle first constructs the complete threshold mask, then finds maximal
runs and reduces each run. Omega instead incrementally opens/closes regions.
This script freezes fixtures and expected results before compiler execution.
"""
from fractions import Fraction
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parent
DATA = ROOT / 'data'


def rounded(value):
    scaled = value * 1000
    return (scaled.numerator * 2 + scaled.denominator) // (scaled.denominator * 2) if scaled >= 0 else -rounded(-value)


def decimal(value):
    return ('-' if value < 0 else '') + f'{abs(value):09d}'


def calculate(raw):
    values = list(map(Fraction, raw))
    samples = values[:16]
    left, right, threshold, origin, spacing, shift, tolerance = values[16:23]
    lines = values[23:]
    weights = [max(Fraction(0), samples[index] - left - (right-left)*Fraction(index, 15)) for index in range(16)]
    mask = [weight >= threshold for weight in weights]
    starts = [index for index in range(16) if mask[index] and (index == 0 or not mask[index-1])]
    ends = [index for index in range(16) if mask[index] and (index == 15 or not mask[index+1])]
    rows = []
    for start, end in zip(starts, ends):
        area = sum(weights[start:end+1])
        centroid = sum(weights[index]*(origin+spacing*index) for index in range(start, end+1))/area + shift
        candidates = [index+1 for index, line in enumerate(lines) if abs(centroid-line) <= tolerance]
        nearest = min(candidates, key=lambda identifier: (abs(centroid-lines[identifier-1]), identifier)) if candidates else 0
        rows.append(dict(start=start, end=end, area=str(area), centroid=str(centroid), candidates=candidates, nearest=nearest))
    output = f'regions={len(rows):09d}\n'
    for row in rows:
        status = ['missing', 'unique', 'ambiguous'][min(len(row['candidates']), 2)]
        ids = ''.join(f'{identifier:09d},' for identifier in row['candidates'])
        output += (f"region start={row['start']:09d} end={row['end']:09d} "
                   f"area_milli={decimal(rounded(Fraction(row['area'])))} "
                   f"centroid_micro={decimal(rounded(Fraction(row['centroid'])))} "
                   f"candidates={len(row['candidates']):09d} nearest={row['nearest']:09d} status={status} ids={ids}\n")
    return rows, output


def inventory(residual, left=10, right=25, threshold=2, origin=500000,
              spacing=1000, shift=0, tolerance=150,
              lines=(503000, 510600, 510800, 520000)):
    # Integer fixtures can have a fractional baseline without altering the oracle.
    baseline = [left + (right-left)*Fraction(index, 15) for index in range(16)]
    samples = [int(value + baseline[index]) for index, value in enumerate(residual)]
    return samples + [left, right, threshold, origin, spacing, shift, tolerance] + list(lines)


def freeze():
    DATA.mkdir(exist_ok=True)
    isolated = [0, 0, 2, 6, 2, 0, 1, 0, 0, 0, 4, 8, 0, 0, 0, 0]
    cases = {
        'isolated': inventory(isolated),
        'overlap': inventory([0, 0, 3, 7, 8, 7, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0], tolerance=1100, lines=(503000, 504000, 505000, 520000)),
        'threshold_edge': inventory([1, 2, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], lines=(501000, 504000, 509000, 520000)),
        'noise_only': inventory([1]*16),
        'missing_catalog': inventory(isolated, lines=(600000, 700000, 800000, 900000)),
        'shift_uncorrected': inventory(isolated, origin=501000),
        'shift_corrected': inventory(isolated, origin=501000, shift=-1000),
        'equal_distance': inventory([0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], tolerance=100, lines=(502900, 503100, 503100, 520000)),
        'capacity_tail': inventory([0, 2]*8, tolerance=0, lines=(501000, 505000, 511000, 515000)),
        'fractional_baseline': inventory(isolated, left=10, right=20, tolerance=300),
        'negative_axis': inventory([3]+[0]*15, origin=0, spacing=1, shift=-5, tolerance=5, lines=(0, 5, 10, 15)),
        'clipped_baseline': [0]*16 + [10, 25, 2, 500000, 1000, 0, 150, 503000, 510600, 510800, 520000],
    }
    expected = {'coordinate_unit': 'milli-nanometer', 'centroid_output_unit': 'micro-nanometer',
                'numeric_tolerance_milli_nm': 0.000501, 'area_tolerance_counts': 0.000501,
                'discrete_oracles': 'exact', 'cases': {}}
    for name, raw in cases.items():
        input_text = ' '.join(map(str, raw)) + '\n'
        rows, output = calculate(raw)
        (DATA / f'{name}.input').write_text(input_text)
        (DATA / f'{name}.expected').write_text(output)
        expected['cases'][name] = dict(exit=0, rows=rows, input_sha256=hashlib.sha256(input_text.encode()).hexdigest(), output_sha256=hashlib.sha256(output.encode()).hexdigest())
    seed = cases['isolated']
    invalid = {'empty': '', 'short': ' '.join(map(str, seed[:-1]))+'\n',
               'extra': ' '.join(map(str, seed+[0]))+'\n', 'nondigit': 'x\n',
               'bare_minus': '-\n', 'sign_eof': '-', 'overflow': '2147483648\n',
               'magnitude': '1000001\n', 'embedded_sign': '1-2\n'}
    for name, position, value in [('negative_intensity', 0, -1), ('large_intensity', 0, 1001),
                                  ('zero_threshold', 18, 0), ('negative_tolerance', 22, -1),
                                  ('zero_spacing', 20, 0), ('bad_baseline', 16, -1),
                                  ('large_catalog', 23, 1000001), ('negative_catalog', 23, -1)]:
        raw = seed.copy()
        raw[position] = value
        invalid[name] = ' '.join(map(str, raw))+'\n'
    for name, text in invalid.items():
        name = 'invalid_'+name
        (DATA / f'{name}.input').write_text(text)
        (DATA / f'{name}.expected').write_text('invalid spectrum input\n')
        expected['cases'][name] = dict(exit=2, rows=[], input_sha256=hashlib.sha256(text.encode()).hexdigest(), output_sha256=hashlib.sha256(b'invalid spectrum input\n').hexdigest())
    # A separate handwritten analytic check prevents silently trusting only code.
    assert expected['cases']['isolated']['rows'][0]['centroid'] == '503000'
    assert expected['cases']['isolated']['rows'][1]['centroid'] == '1532000/3'
    assert expected['cases']['overlap']['rows'][0]['area'] == '28'
    assert expected['cases']['capacity_tail']['rows'][-1]['end'] == 15
    assert expected['cases']['equal_distance']['rows'][0]['nearest'] == 1
    assert len(expected['cases']['noise_only']['rows']) == 0
    (DATA / 'oracle.json').write_text(json.dumps(expected, indent=2)+'\n')
    digest = hashlib.sha256((DATA / 'oracle.json').read_bytes()).hexdigest()
    (DATA / 'oracle.sha256').write_text(digest+'  oracle.json\n')
    print(f'Frozen {len(cases)} valid and {len(invalid)} invalid cases; oracle sha256 {digest}')

if __name__ == '__main__':
    freeze()
