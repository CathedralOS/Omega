#!/usr/bin/env python3
"""Run frozen oracles against a compiled executable, without regenerating them."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('executable')
    parser.add_argument('--output', default=str(ROOT / 'build/validation'))
    arguments = parser.parse_args()
    output = Path(arguments.output)
    output.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((ROOT / 'data/oracle_manifest.json').read_text())
    assert hashlib.sha256((ROOT / 'data/cases.json').read_bytes()).hexdigest() == manifest['cases_sha256']
    records = []
    cases = [(case, (ROOT / f"data/{case['name']}.stdin").read_bytes(),
              (ROOT / f"data/{case['name']}.stdout").read_bytes()) for case in manifest['cases']]
    wide = json.loads((ROOT / 'data/wide_bands.oracle.json').read_text())
    wide_input = (ROOT / 'data/wide_bands.stdin').read_bytes()
    assert hashlib.sha256(wide_input).hexdigest() == wide['input_sha256']
    cases.append((wide, wide_input, (ROOT / 'data/wide_bands.stdout').read_bytes()))
    unique_input = (ROOT / 'data/unique.stdin').read_bytes()
    unique_output = (ROOT / 'data/unique.stdout').read_bytes()
    # These protocol/window variants retain an already frozen numerical oracle.
    for name, payload in [('eof_without_lf', unique_input.rstrip(b'\n')),
                          ('crlf', unique_input.replace(b'\n', b'\r\n')),
                          ('tabs', unique_input.replace(b' ', b'\t')),
                          ('maximum_window', unique_input.replace(b'-8 8 -8 8', b'-20 20 -20 20'))]:
        cases.append(({'name': name, 'exit': 0, 'stdout_sha256': manifest['cases'][0]['stdout_sha256']}, payload, unique_output))
    error_output = (ROOT / 'data/bad_count.stdout').read_bytes()
    cases.append(({'name': 'blank_line', 'exit': 64, 'stdout_sha256': hashlib.sha256(error_output).hexdigest()}, b'\n', error_output))
    for case, payload, expected in cases:
        name = case['name']
        assert hashlib.sha256(expected).hexdigest() == case['stdout_sha256']
        actual = subprocess.run([str(Path(arguments.executable).resolve())],
                                input=payload,
                                capture_output=True, timeout=30)
        (output / f'{name}.stdin').write_bytes(payload)
        (output / f'{name}.stdout').write_bytes(actual.stdout)
        (output / f'{name}.stderr').write_bytes(actual.stderr)
        matches = actual.returncode == case['exit'] and actual.stdout == expected and not actual.stderr
        records.append({'case': name, 'exit': actual.returncode, 'expected_exit': case['exit'],
                        'stdout_sha256': hashlib.sha256(actual.stdout).hexdigest(), 'passed': matches})
        print(name, 'PASS' if matches else 'FAIL', actual.returncode)
    demo = subprocess.run([str(Path(arguments.executable).resolve())], input=b'', capture_output=True, timeout=30)
    assert demo.returncode == 0 and demo.stdout == (ROOT / 'data/unique.stdout').read_bytes() and not demo.stderr
    (output / 'default_demo.stdout').write_bytes(demo.stdout)
    (output / 'default_demo.stderr').write_bytes(demo.stderr)
    records.append({'case': 'default_demo', 'exit': demo.returncode, 'expected_exit': 0,
                    'stdout_sha256': hashlib.sha256(demo.stdout).hexdigest(), 'passed': True})
    (output / 'results.json').write_text(json.dumps(records, indent=2) + '\n')
    assert all(record['passed'] for record in records), 'frozen oracle mismatch'

if __name__ == '__main__':
    main()
