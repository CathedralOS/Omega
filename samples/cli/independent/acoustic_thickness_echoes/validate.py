#!/usr/bin/env python3
"""Sequential public-entrypoint validation against pre-frozen fixtures."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--compiler', required=True)
parser.add_argument('--only', choices=['check', 'native', 'both', 'invalid'])
options = parser.parse_args()
project = Path(__file__).resolve().parent
repository = project.parents[3]
binary = Path(options.compiler).resolve()
artifacts = project / 'build' / 'validation'
artifacts.mkdir(parents=True, exist_ok=True)
environment = dict(os.environ, OMEGA_LIBRARY_ROOT=str(repository / 'source/library'))
oracle_bytes = (project / 'data/oracle.json').read_bytes()
assert hashlib.sha256(oracle_bytes).hexdigest() == '5b0dc70c24f6aa0755767fa16f45f3733e831bf2533cf288603ed9952e9ad4f2', 'frozen oracle identity changed'
oracle = json.loads(oracle_bytes)
rows = []
def run(name, arguments, stdin=b''):
    command = [str(binary), *map(str, arguments)]
    observed = subprocess.run(command, input=stdin, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, cwd=repository,
                              env=environment, timeout=240)
    (artifacts / f'{name}.stdout').write_bytes(observed.stdout)
    (artifacts / f'{name}.stderr').write_bytes(observed.stderr)
    rows.append({'name': name, 'command': command, 'exit': observed.returncode})
    (artifacts / 'commands.json').write_text(json.dumps(rows, indent=2) + '\n')
    return observed

for name, expected in oracle['cases'].items():
    for suffix in ['stdin', 'stdout']:
        payload = (project / 'data' / f'{name}.{suffix}').read_bytes()
        assert hashlib.sha256(payload).hexdigest() == expected[f'{suffix}_sha256'], name
if options.only in [None, 'check']:
    result = run('check', ['--check', '--target', 'linux_x86_64', project / 'main.omg'])
    assert result.returncode == 0, result.stderr.decode(errors='replace')
if options.only in [None, 'native', 'both']:
    both = options.only == 'both'
    for name, expected in oracle['cases'].items():
        arguments = ['run', '--keep']
        if both:
            arguments.append('--both')
        arguments.append(project / 'main.omg')
        result = run(f'{"both" if both else "native"}-{name}', arguments,
                     (project / 'data' / f'{name}.stdin').read_bytes())
        assert result.returncode == expected['exit'], (name, result.returncode, result.stderr)
        assert result.stdout == (project / 'data' / f'{name}.stdout').read_bytes(), (name, result.stdout)
        if both:
            assert b'declin' not in result.stderr.lower(), (name, result.stderr)
if options.only is None:
    for name in oracle['cases']:
        expected = oracle['cases'][name]
        result = run(f'both-{name}', ['run', '--both', '--keep', project / 'main.omg'],
                     (project / 'data' / f'{name}.stdin').read_bytes())
        assert result.returncode == expected['exit'], (name, result.returncode, result.stderr)
        assert result.stdout == (project / 'data' / f'{name}.stdout').read_bytes(), (name, result.stdout)
        assert b'declin' not in result.stderr.lower(), (name, result.stderr)
if options.only in [None, 'invalid']:
    result = run('invalid', ['--check', project / 'controls/invalid_bounds/main.omg'])
    text = result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')
    assert result.returncode == 1, (result.returncode, text)
    for fragment in (project / 'controls/invalid_bounds/expected.txt').read_text().splitlines():
        assert fragment in text, text
print(f'validated {len(rows)} sequential commands')
