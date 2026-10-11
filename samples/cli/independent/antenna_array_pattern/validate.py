#!/usr/bin/env python3
"""Serial public-CLI and native validation; never accepts package decisions."""
import argparse, hashlib, json, os, subprocess, sys, time
from pathlib import Path
from reference import check
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[3]

parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
parser.add_argument('--native', help='Already published native executable (skip check/build/both)')
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
    row = dict(name=name, command=command, stdin=str(fixture) if fixture else None,
               exit=completed.returncode, expected_exit=expected, seconds=time.monotonic()-started)
    results.append(row)
    (output / 'results.json').write_text(json.dumps(results, indent=2)+'\n')
    print(json.dumps(row), flush=True)
    assert completed.returncode == expected, completed.stderr.decode(errors='replace')
    return completed

if options.native:
    native = Path(options.native).resolve()
else:
    run('check', [compiler, '--check', '--target', 'linux_x86_64', str(ROOT/'main.omg')])
    native_directory = output / 'native'
    run('compile', [compiler, '--target', 'linux_x86_64', '--build-dir', str(native_directory), str(ROOT/'main.omg')])
    products = [p for p in native_directory.rglob('*') if p.is_file() and os.access(p,os.X_OK) and p.read_bytes()[:4]==b'\x7fELF']
    assert len(products)==1, products
    native = products[0]
    run('both_suite', [compiler, 'run', '--both', '--keep', str(ROOT/'main.omg')], ROOT/'data/suite.stdin')
    stderr=(output/'both_suite.stderr').read_text()
    assert 'interp exit: 0' in stderr and 'DECLINED' not in stderr, stderr
    check(output/'both_suite.stdout', None)

for name,selection in [('suite',None),('suite_selector',None),('suite_newline',None),('broadside',0),('steering',1),('zero',2),('duplicate',3),('taper',4)]:
    run(name,[str(native)],ROOT/f'data/{name}.stdin')
    check(output/f'{name}.stdout',selection)
for name,status,diagnostic in [('bad_position',65,'ERROR geometry:'),('bad_weight',65,'ERROR geometry:'),
                               ('bad_selector',64,'ERROR input:'),('trailing_junk',64,'ERROR input:'),('suite_junk',64,'ERROR input:'),
                               ('trailing_line_junk',64,'ERROR input:'),('newline_junk',64,'ERROR input:')]:
    completed=run(name,[str(native)],ROOT/f'data/{name}.stdin',status)
    assert diagnostic in completed.stdout.decode() and ' complete' not in completed.stdout.decode()
print('PASS native fixtures, stdout oracles and rejected runtime inputs', flush=True)
print('native sha256',hashlib.sha256(native.read_bytes()).hexdigest(),flush=True)
