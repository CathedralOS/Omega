"""Compare captured application artifacts with previously frozen oracles."""
from pathlib import Path
import hashlib
import json
import re
import sys

ROOT = Path(__file__).resolve().parents[1]

def verify_frozen():
    hashes = json.loads((ROOT / 'data/frozen.sha256.json').read_text())
    for name, digest in hashes.items():
        assert hashlib.sha256((ROOT / 'data' / name).read_bytes()).hexdigest() == digest, f'frozen oracle changed: {name}'


def compare(name, actual, exit_code):
    tolerances = json.loads((ROOT / 'data/tolerances.json').read_text())
    if name.startswith('invalid_'):
        assert exit_code == tolerances['invalid_exit'], (name, exit_code)
        assert actual == (ROOT / 'data/invalid.expected.txt').read_text(), (name, actual)
        return
    assert exit_code == tolerances['valid_exit'], (name, exit_code)
    expected = (ROOT / 'data' / f'{name}.expected.txt').read_text()
    left, right = actual.splitlines(), expected.splitlines()
    assert len(left) == len(right), (name, len(left), len(right), actual)
    numeric = re.compile(r'(-?\d+)$')
    absolute = tolerances['absolute_micro_units']
    relative = tolerances['relative']
    for number, (observed, frozen) in enumerate(zip(left, right), 1):
        actual_tokens, expected_tokens = observed.split(), frozen.split()
        assert len(actual_tokens) == len(expected_tokens), (name, number, observed, frozen)
        for token, goal in zip(actual_tokens, expected_tokens):
            if '=' not in goal or not numeric.search(goal):
                assert token == goal, (name, number, token, goal)
                continue
            key, target = goal.split('=')
            observed_key, value = token.split('=')
            assert observed_key == key, (name, number, token, goal)
            target, value = int(target), int(value)
            if key == 'wetted':
                assert target == value, (name, number, token, goal)
                continue
            quantity = 'geometry'
            for word in ('area', 'force', 'moment', 'center'):
                if key.startswith(word):
                    quantity = word
            bound = absolute[quantity] + relative * abs(target)
            assert abs(value - target) <= bound, (name, number, token, goal, bound)
    # Verify emitted loads add up and center agrees with the signed hinge moment.
    # Rounding gets one additional micro-unit per segment in conservation checks.
    gamma, hinge_mm = None, None
    input_words = (ROOT / 'data' / f'{name}.in').read_text().split()
    hinge = int(input_words[3]) / 1000
    accumulated = {'area_micro_m2': 0, 'force_uN': 0, 'moment_uNm': 0}
    spans = 0
    for line in left:
        fields = dict(item.split('=') for item in line.split() if '=' in item)
        if line.startswith('LEVEL'):
            accumulated = dict.fromkeys(accumulated, 0)
            spans = 0
        elif line.startswith('WET'):
            spans += 1
            assert int(fields['upper_um']) > int(fields['lower_um'])
            for key in accumulated:
                accumulated[key] += int(fields[key])
        elif line.startswith('TOTAL'):
            assert int(fields['wetted']) == spans
            for key, total in accumulated.items():
                observed = int(fields[key])
                assert abs(observed - total) <= spans * 4 + relative * abs(total), (name, key, observed, total)
            force = int(fields['force_uN'])
            if force:
                reconstructed_center = hinge * 1000000 + int(fields['moment_uNm']) / force * 1000000
                moment = abs(int(fields['moment_uNm']))
                propagated_rounding = 2 + 500000 / force + moment * 500000 / (force * force)
                assert abs(int(fields['center_um']) - reconstructed_center) <= propagated_rounding, (name, 'center-moment relation')
            else:
                assert fields['center_um'] == 'undefined'

if __name__ == '__main__':
    verify_frozen()
    name, output_file, exit_code = sys.argv[1:]
    compare(name, Path(output_file).read_text(), int(exit_code))
    print(f'{name}: artifact oracle, geometry, conservation and center checks passed')
