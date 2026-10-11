#!/usr/bin/env python3
"""Serial, byte-exact public CLI validation; never author or update oracles."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True, type=Path)
parser.add_argument('--timeout', type=int, default=180)
options = parser.parse_args()
project = Path(__file__).resolve().parent
repository = project.parents[3]
output = project / 'build/validation'
output.mkdir(parents=True, exist_ok=True)
environment = dict(os.environ, OMEGA_LIBRARY_ROOT=str(repository / 'source/library'))
for row in (project / 'data/FROZEN.sha256').read_text().splitlines():
    expected, name = row.split('  ', 1)
    assert hashlib.sha256((project / 'data' / name).read_bytes()).hexdigest() == expected, name
results = []
def invoke(label, arguments, stdin=b''):
    command = [str(options.omega.resolve()), *map(str, arguments)]
    completed = subprocess.run(command, input=stdin, capture_output=True,
                               cwd=repository, env=environment, timeout=options.timeout)
    (output / f'{label}.stdout').write_bytes(completed.stdout)
    (output / f'{label}.stderr').write_bytes(completed.stderr)
    result = dict(label=label, command=command, exit=completed.returncode)
    results.append(result)
    (output / 'results.json').write_text(json.dumps(results, indent=2)+'\n')
    return completed
invalid = invoke('bounds', ['--check', project / 'controls/bounds/main.omg'])
fragment = (project / 'controls/bounds/expected.txt').read_text().strip().encode()
assert invalid.returncode == 1 and fragment in invalid.stderr + invalid.stdout, 'wrong-reason bounds rejection'
checked = invoke('check', ['--check', project / 'main.omg'])
assert checked.returncode == 0, 'check incomplete/rejected; see exact stderr'
cases = json.loads((project / 'data/oracle.json').read_text())
for route, flags in [('native', []), ('terminal', ['--both'])]:
    for name, oracle in cases.items():
        completed = invoke(f'{route}-{name}', ['run', *flags, '--keep', project / 'main.omg'],
                           (project / 'data' / f'{name}.stdin').read_bytes())
        assert completed.returncode == oracle['exit'], f'{route}/{name}: exit {completed.returncode}'
        assert completed.stdout == (project / 'data' / f'{name}.stdout').read_bytes(), f'{route}/{name}: stdout mismatch'
        if route == 'terminal':
            assert f'interp exit: {oracle["exit"]}'.encode() in completed.stderr, f'{name}: no supported Terminal exit agreement'
        print(f'{route}/{name}: exact stdout and exit {completed.returncode}', flush=True)
print('18 cases in each route plus check and separate intended rejection passed')
