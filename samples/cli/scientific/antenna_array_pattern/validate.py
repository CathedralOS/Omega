#!/usr/bin/env python3
"""Validate frozen numerical oracles and selector errors through public execution."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

from reference import check, checked_oracle

ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[3]
VALID_CASES = [('suite', None), ('suite_selector', None), ('suite_newline', None),
               ('broadside', 0), ('steering', 1), ('zero', 2), ('duplicate', 3), ('taper', 4)]
INVALID_CASES = [('bad_position', 65, 'ERROR geometry:'), ('bad_weight', 65, 'ERROR geometry:'),
                 ('bad_selector', 64, 'ERROR input:'), ('trailing_junk', 64, 'ERROR input:'),
                 ('suite_junk', 64, 'ERROR input:'), ('trailing_line_junk', 64, 'ERROR input:'),
                 ('newline_junk', 64, 'ERROR input:')]

parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
parser.add_argument('--native', help='Published native executable; native-only replay without compilation')
options = parser.parse_args()
compiler = str(Path(options.omega).resolve())
output = ROOT / 'build/validation'
output.mkdir(parents=True, exist_ok=True)
results = []
environment = dict(os.environ, OMEGA_LIBRARY_ROOT=str(REPOSITORY / 'source/library'))


def run(name, command, fixture=None, expected=0):
    started = time.monotonic()
    completed = subprocess.run(command, input=fixture.read_bytes() if fixture else b'',
                               capture_output=True, env=environment, cwd=REPOSITORY, timeout=600)
    (output / f'{name}.stdout').write_bytes(completed.stdout)
    (output / f'{name}.stderr').write_bytes(completed.stderr)
    results.append(dict(name=name, command=command, stdin=str(fixture) if fixture else None,
                        exit=completed.returncode, expected_exit=expected,
                        seconds=time.monotonic()-started))
    (output / 'results.json').write_text(json.dumps(results, indent=2)+'\n')
    print(json.dumps(results[-1]), flush=True)
    assert completed.returncode == expected, completed.stderr.decode(errors='replace')
    return completed


checked_oracle()
if options.native:
    native = Path(options.native).resolve()
    for name, selection in VALID_CASES:
        run(name, [str(native)], ROOT/f'data/{name}.stdin')
        check(output/f'{name}.stdout', selection)
    for name, status, diagnostic in INVALID_CASES:
        completed = run(name, [str(native)], ROOT/f'data/{name}.stdin', status)
        assert diagnostic in completed.stdout.decode() and ' complete' not in completed.stdout.decode()
    print('native sha256', hashlib.sha256(native.read_bytes()).hexdigest(), flush=True)
else:
    # One compilation serves every finite input. Check the actual Terminal streams
    # as well as its status: exit agreement alone does not establish the model.
    cases = output / 'cases'
    completed = run('both_cases', [compiler, 'run', '--both', '--keep', '--inputs',
                                  str(ROOT/'data'), '--outputs', str(cases), str(ROOT/'main.omg')])
    with (cases/'results.tsv').open() as stream:
        rows = list(csv.reader(stream, delimiter='\t'))
    observed = {row[0]: row[1:] for row in rows[1:]}
    expected_names = {name for name, _ in VALID_CASES} | {name for name, _, _ in INVALID_CASES}
    assert len(rows)-1 == len(observed) == len(expected_names) == 15, rows
    assert set(observed) == expected_names, observed
    for name, selection in VALID_CASES:
        assert observed[name] == ['0', 'agrees', '0'], (name, observed[name])
        check(cases/f'{name}.stdout', selection)
        check(cases/f'{name}.terminal.stdout', selection)
    for name, status, diagnostic in INVALID_CASES:
        assert observed[name] == [str(status), 'agrees', str(status)], (name, observed[name])
        for suffix in ('stdout', 'terminal.stdout'):
            text = (cases/f'{name}.{suffix}').read_text()
            assert diagnostic in text and ' complete' not in text, (name, suffix, text)
    for name in expected_names:
        assert (cases/f'{name}.stdout').read_bytes() == (cases/f'{name}.terminal.stdout').read_bytes(), name
        assert (cases/f'{name}.stderr').read_bytes() == (cases/f'{name}.terminal.stderr').read_bytes() == b'', name
    print('PASS 15 native and Terminal cases, frozen stdout oracles and rejected inputs', flush=True)
