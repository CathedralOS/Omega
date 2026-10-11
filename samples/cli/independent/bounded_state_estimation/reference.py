"""Independent exact-rational oracle, never application execution.

Condition the bivariate Gaussian via P - (P h)(h' P)/S, rather than
copying the application's Joseph matrix pipeline. Missing rows have no update.
The committed expected.json is frozen before any Omega execution.
"""
from fractions import Fraction as F
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parent

def calculate(text):
    lines = text.splitlines(keepends=True)
    if not lines or len(lines[0]) != 8 or not lines[0].endswith('\n'):
        return {'exit': 64, 'error': 'input', 'trace': []}
    header = lines[0]
    if header[0:2] != 'C ' or header[3] != ' ' or header[5] != ' ' or header[2] not in '123' or header[4] not in '0123' or header[6] not in '0123':
        return {'exit': 64, 'error': 'input', 'trace': []}
    interval, noise, variance = map(F, (header[2], header[4], header[6]))
    position, velocity = F(0), F(1)
    a, b, d = F(1), F(0), F(1)
    rows = []
    for line in lines[1:]:
        if not line.endswith('\n') or len(line) >= 16:
            return {'exit': 64, 'error': 'input', 'trace': rows}
        if len(rows) == 8:
            return {'exit': 66, 'error': 'capacity', 'trace': rows}
        missing = line == 'M\n'
        if not missing and not (len(line) == 5 and line[0] in 'PV' and line[1] == ' ' and line[2] in '+-' and line[3] in '0123456789'):
            return {'exit': 64, 'error': 'input', 'trace': rows}
        position += interval * velocity
        a, b, d = a + 2*interval*b + interval*interval*d + noise, b + interval*d, d + noise
        innovation = innovation_variance = F(0)
        policy = 'missing'
        singular = False
        if not missing:
            measured = F(line[2:4])
            position_axis = line[0] == 'P'
            innovation = measured - (position if position_axis else velocity)
            projected_position, projected_velocity = (a, b) if position_axis else (b, d)
            innovation_variance = (a if position_axis else d) + variance
            singular = innovation_variance <= F(1, 10**12)
            if not singular:
                position += projected_position * innovation / innovation_variance
                velocity += projected_velocity * innovation / innovation_variance
                a, b, d = (a-projected_position**2/innovation_variance,
                           b-projected_position*projected_velocity/innovation_variance,
                           d-projected_velocity**2/innovation_variance)
                policy = 'update'
        assert a >= 0 and d >= 0 and a*d-b*b >= 0
        values = [position, velocity, a, b, b, d, innovation, innovation_variance]
        rows.append({'step': len(rows)+1, 'policy': policy,
                     'exact': [str(v) for v in values], 'values': [float(v) for v in values]})
        if singular:
            return {'exit': 65, 'error': 'singular', 'trace': rows}
    return {'exit': 0 if rows else 64, 'error': None if rows else 'input', 'trace': rows}

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--freeze', action='store_true', required=True)
    parser.parse_args()
    output = {'oracle': 'exact Fraction Gaussian conditioning; frozen before Omega execution',
              'absolute_tolerance': 0.000002, 'relative_tolerance': 1e-9,
              'runtime_psd_tolerance': 1e-10, 'singular_threshold': 1e-12,
              'cases': {}}
    for path in sorted((ROOT/'data').glob('*.txt')):
        output['cases'][path.stem] = dict(calculate(path.read_text()), input_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    (ROOT/'expected.json').write_text(json.dumps(output, indent=2)+'\n')
    print('Frozen', len(output['cases']), 'cases')
