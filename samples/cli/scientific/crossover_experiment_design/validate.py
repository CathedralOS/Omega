"""Verify a compiled Omega executable against frozen independent oracles."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

# Validation must not add Python bytecode to the reviewed source package.
sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('executable', type=Path)
parser.add_argument('--omega', type=Path)
parser.add_argument('--terminal', action='store_true')
options = parser.parse_args()
expected_path = ROOT / 'data/expected.json'
assert hashlib.sha256(expected_path.read_bytes()).hexdigest() == '6ce9539d7a9b9479641ee8a26ee414e15e650553fbcc512adea33bb01fda6ab5'
expected = json.loads(expected_path.read_text())
output = ROOT / 'build/validation'
output.mkdir(parents=True, exist_ok=True)
results = []
for name, oracle in expected.items():
    observed = subprocess.run([str(options.executable.resolve())],
                              input=(ROOT / 'data' / (name + '.stdin')).read_bytes(),
                              capture_output=True, timeout=30)
    (output / (name + '.stdout')).write_bytes(observed.stdout)
    (output / (name + '.stderr')).write_bytes(observed.stderr)
    assert observed.returncode == oracle['exit'], (name, observed.returncode, oracle['exit'])
    assert observed.stdout == oracle['stdout'].encode(), (name, observed.stdout, oracle['stdout'])
    assert observed.stderr == b'', (name, observed.stderr)
    if 'allocation' in oracle:
        # Parse real emitted rows, reconstruct counts without using search IDs.
        from reference import accepted, counts
        rows = [tuple(map(int, line.split(':')[1].split())) for line in observed.stdout.decode().splitlines()
                if line.startswith('subject ')]
        treatments, subjects, cap, forbidden = map(int, (ROOT / 'data' / (name + '.stdin')).read_text().strip().split(','))
        assert len(rows) == subjects and accepted(rows, treatments, cap, forbidden)
        assert counts(rows, treatments) == (oracle['period_counts'], oracle['carryover_counts'])
    results.append({'case': name, 'native_exit': observed.returncode, 'stdout_matches': True})
    print(name, 'native exact output and exit verified')
if options.terminal:
    assert options.omega
    for name in ('two_tiny', 'three_latin', 'three_pair_balanced', 'three_reverse_forbidden',
                 'three_cap_infeasible', 'invalid_character'):
        observed = subprocess.run([str(options.omega.resolve()), 'run', '--both', str(ROOT / 'main.omg')],
                                  input=(ROOT / 'data' / (name + '.stdin')).read_bytes(),
                                  capture_output=True, timeout=180)
        (output / (name + '.both.stdout')).write_bytes(observed.stdout)
        (output / (name + '.both.stderr')).write_bytes(observed.stderr)
        assert observed.returncode == expected[name]['exit'], (name, observed.returncode, observed.stderr)
        assert observed.stdout == expected[name]['stdout'].encode(), (name, observed.stdout)
        assert f"interp exit: {expected[name]['exit']}\n".encode() in observed.stderr, (name, observed.stderr)
        assert not any(marker in observed.stderr for marker in (b'DECLINED', b'DIVERGENCE', b'FAILED')), (name, observed.stderr)
        results.append({'case': name, 'terminal_agreement': True})
        print(name, 'Terminal agreement verified (CLI compares exit; native stdout oracle checked separately)')
(output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
