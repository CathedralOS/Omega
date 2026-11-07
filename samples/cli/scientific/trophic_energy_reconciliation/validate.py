#!/usr/bin/env python3
"""Validate frozen fixtures serially; never accepts package review decisions."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
parser.add_argument('--native-image', help='Reuse one published native product for all inputs')
parser.add_argument('--route', choices=['check', 'native', 'both', 'invalid-control'], required=True)
arguments = parser.parse_args()
sample = Path(__file__).resolve().parent
root = sample / 'main.omg'
artifacts = sample / 'build' / 'validation' / arguments.route
artifacts.mkdir(parents=True, exist_ok=True)
oracle = json.loads((sample / 'data/oracle.json').read_text())
records = []

def execute(name, command, data=b''):
    completed = subprocess.run(command, input=data, capture_output=True, timeout=600)
    (artifacts / (name + '.stdout')).write_bytes(completed.stdout)
    (artifacts / (name + '.stderr')).write_bytes(completed.stderr)
    return completed

if arguments.route == 'invalid-control':
    command = [arguments.omega, '--check', str(sample / 'controls/invalid_bounds/main.omg')]
    result = execute('bounds', command)
    diagnostics = result.stdout + result.stderr
    fragment = (sample / 'controls/invalid_bounds/expected.txt').read_bytes().strip()
    error_lines = [line.strip() for line in diagnostics.splitlines() if line.lstrip().startswith(b'error:')]
    bounds_error = re.compile(rb'error: cannot prove index `[^`]+` is within length 3')
    passed = (result.returncode == 1 and fragment in diagnostics
              and b'checked compilation failed for package `trophic_invalid_bounds` with 1 diagnostic(s)' in diagnostics
              and len(error_lines) == 1 and bounds_error.fullmatch(error_lines[0]) is not None)
    records.append({'case': 'bounds', 'exit': result.returncode, 'passed': passed})
elif arguments.route == 'check':
    command = [arguments.omega, '--check', str(root)]
    result = execute('check', command)
    records.append({'case': 'check', 'exit': result.returncode, 'passed': result.returncode == 0})
else:
    command = [arguments.native_image] if arguments.route == 'native' and arguments.native_image else [arguments.omega, 'run'] + (['--both'] if arguments.route == 'both' else []) + [str(root)]
    for name, expected in oracle['cases'].items():
        if name == 'invalid':
            continue
        output = (sample / 'data' / (name + '.stdout')).read_bytes()
        assert hashlib.sha256(output).hexdigest() == expected['stdout_sha256'], 'oracle changed'
        result = execute(name, command, (sample / 'data' / (name + '.stdin')).read_bytes())
        passed = result.returncode == expected['exit'] and result.stdout == output
        comparison = result.stderr.decode(errors='replace')
        if arguments.route == 'both':
            passed = passed and f"interp exit: {expected['exit']}" in comparison and 'DECLINED' not in comparison and 'DIVERGENCE' not in comparison
        records.append({'case': name, 'exit': result.returncode, 'stdout_exact': result.stdout == output, 'passed': passed})
    for name in oracle['cases']['invalid']['cases']:
        result = execute('invalid_' + name, command, (sample / 'data' / ('invalid_' + name + '.stdin')).read_bytes())
        passed = result.returncode == 64 and result.stdout.endswith(b'INVALID INPUT\n') and b'FEASIBLE\n' not in result.stdout
        if arguments.route == 'both':
            passed = passed and b'interp exit: 64' in result.stderr and b'DECLINED' not in result.stderr and b'DIVERGENCE' not in result.stderr
        records.append({'case': 'invalid_' + name, 'exit': result.returncode, 'passed': passed})
report = {'route': arguments.route, 'command': command, 'omega': arguments.omega, 'library_root': os.environ.get('OMEGA_LIBRARY_ROOT'), 'records': records}
(artifacts / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
raise SystemExit(0 if all(record['passed'] for record in records) else 1)
