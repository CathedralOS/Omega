#!/usr/bin/env python3
"""Independent oracle: intersect each anchor's admissible lattice set.

This deliberately differs from Omega's point-first rejection scan. The exact
unique and mirror fixtures are additionally proved by continuous intersections
of the first two circles, then testing the third. No compiler is invoked here.
"""
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent

def solve(text):
    values = list(map(int, text.split()))
    xmin, xmax, ymin, ymax, count = values[:5]
    anchors = [tuple(values[5 + 4 * index:9 + 4 * index]) for index in range(count)]
    window = {(x, y) for x in range(xmin, xmax + 1) for y in range(ymin, ymax + 1)}
    admitted = window.copy()
    for ax, ay, lower, upper in anchors:
        annulus = {(ax + dx, ay + dy) for dx in range(-upper, upper + 1)
                   for dy in range(-upper, upper + 1)
                   if lower ** 2 <= dx ** 2 + dy ** 2 <= upper ** 2}
        admitted &= annulus
    points = sorted(admitted, key=lambda point: (point[1], point[0]))
    same_center = len({(row[0], row[1]) for row in anchors}) == 1
    status = ('inconsistent' if not points else 'degenerate' if same_center else
              'unique' if len(points) == 1 else 'ambiguous')
    rows = []
    for x, y in points:
        residuals = [(x - ax) ** 2 + (y - ay) ** 2 - lower ** 2
                     for ax, ay, lower, upper in anchors]
        rows.append(f'candidate {x} {y} residual2 ' + ' '.join(map(str, residuals)))
    rows.extend([f'count {len(points)}', f'status {status}'])
    return points, status, '\n'.join(rows) + '\n'

def circle_intersections(first, second):
    ax, ay, radius = first
    bx, by, other = second
    distance = math.hypot(bx - ax, by - ay)
    projection = (radius * radius - other * other + distance * distance) / (2 * distance)
    height = math.sqrt(radius * radius - projection * projection)
    ux, uy = (bx - ax) / distance, (by - ay) / distance
    return [(ax + projection * ux - height * uy, ay + projection * uy + height * ux),
            (ax + projection * ux + height * uy, ay + projection * uy - height * ux)]

def freeze():
    cases_path = ROOT / 'data/cases.json'
    cases = json.loads(cases_path.read_text())
    for case in cases:
        if case.get('error'):
            expected, exit_code = 'error invalid input\n', 64
        else:
            points, status, expected = solve(case['input'])
            assert list(map(list, points)) == case['points'], case['name']
            assert status == case['status'], case['name']
            exit_code = 0
        (ROOT / f"data/{case['name']}.stdin").write_bytes(case['input'].encode())
        (ROOT / f"data/{case['name']}.stdout").write_bytes(expected.encode())
        case['exit'] = exit_code
    intersections = circle_intersections((0, 0, 5), (6, 0, 5))
    assert set(intersections) == {(3.0, 4.0), (3.0, -4.0)}
    assert [point for point in intersections if math.hypot(point[0] - 3, point[1] - 1) == 3] == [(3.0, 4.0)]
    manifest = {'strategy': 'anchor-first annulus set intersection plus continuous circle witnesses',
                'coordinate_model': 'closed integer lattice, unit spacing',
                'integer_output_tolerance': 0, 'circle_witness_tolerance': 1e-12,
                'cases_sha256': hashlib.sha256(cases_path.read_bytes()).hexdigest(),
                'cases': [{'name': case['name'], 'exit': case['exit'],
                           'stdout_sha256': hashlib.sha256((ROOT / f"data/{case['name']}.stdout").read_bytes()).hexdigest()}
                          for case in cases]}
    (ROOT / 'data/oracle_manifest.json').write_bytes((json.dumps(manifest, indent=2) + '\n').encode())
    wide_input = (ROOT / 'data/wide_bands.stdin').read_text()
    wide_points, wide_status, wide_output = solve(wide_input)
    assert set(wide_points) == {(x, y) for x in range(11) for y in range(11)}
    assert wide_status == 'ambiguous'
    (ROOT / 'data/wide_bands.stdout').write_bytes(wide_output.encode())
    wide_manifest = {'name': 'wide_bands', 'exit': 0,
                     'input_sha256': hashlib.sha256(wide_input.encode()).hexdigest(),
                     'stdout_sha256': hashlib.sha256(wide_output.encode()).hexdigest(),
                     'count': 121, 'integer_output_tolerance': 0}
    (ROOT / 'data/wide_bands.oracle.json').write_bytes((json.dumps(wide_manifest, indent=2) + '\n').encode())
    print(f"froze {len(cases)} cases + wide bands (121 points); circle intersections {intersections}; integer tolerance 0")

if __name__ == '__main__':
    freeze()
