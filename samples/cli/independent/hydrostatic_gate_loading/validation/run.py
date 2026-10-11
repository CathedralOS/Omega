"""Serial, fail-closed current-compiler acceptance driver.

Package review must be completed first. This script never accepts package
policy, changes oracles, or runs more than one Omega process at a time.
"""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys
import time
sys.dont_write_bytecode = True
from verify import ROOT, compare, verify_frozen

REPOSITORY = ROOT.parents[3]
COMPILER = Path(sys.argv[1]).resolve()
ARTIFACTS = ROOT / 'build' / 'validation'
ARTIFACTS.mkdir(parents=True, exist_ok=True)
ENVIRONMENT = dict(os.environ, OMEGA_LIBRARY_ROOT=str(REPOSITORY / 'source/library'))
records = []

def record(label, command, input_bytes=None):
    started = time.monotonic()
    process = subprocess.run([str(p) for p in command], input=input_bytes, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=ENVIRONMENT, cwd=REPOSITORY, timeout=300)
    (ARTIFACTS / f'{label}.stdout').write_bytes(process.stdout)
    (ARTIFACTS / f'{label}.stderr').write_bytes(process.stderr)
    records.append(dict(label=label, command=[str(p) for p in command], exit=process.returncode, seconds=round(time.monotonic()-started, 3), stdout_sha256=hashlib.sha256(process.stdout).hexdigest(), stderr_sha256=hashlib.sha256(process.stderr).hexdigest()))
    (ARTIFACTS / 'commands.json').write_text(json.dumps(records, indent=2) + '\n')
    print(f'{label}: exit={process.returncode}', flush=True)
    return process

verify_frozen()
check = record('check', [COMPILER, '--check', '--target', 'linux_x86_64', ROOT / 'main.omg'])
assert check.returncode == 0, check.stderr.decode()
build = record('native_build', [COMPILER, '--target', 'linux_x86_64', '--build-dir', ROOT / 'build/native', ROOT / 'main.omg'])
assert build.returncode == 0, build.stderr.decode()
published = [line.removeprefix('published native output to ') for line in build.stdout.decode().splitlines() if line.startswith('published native output to ')]
assert len(published) == 1, build.stdout.decode()
executable = Path(published[0])
if not executable.is_absolute():
    executable = REPOSITORY / executable
assert executable.is_file()
for fixture in sorted((ROOT / 'data').glob('*.in')):
    name = fixture.stem
    outcome = record('native_' + name, [executable], fixture.read_bytes())
    compare(name, outcome.stdout.decode(), outcome.returncode)
    assert outcome.stderr == b'', outcome.stderr
    print(f'{name}: frozen output checks passed', flush=True)
for name in ['rectangle', 'triangle', 'piecewise', 'capacity', 'millimetre', 'maximum', 'invalid_duplicate', 'invalid_digit_overflow']:
    outcome = record('terminal_' + name, [COMPILER, 'run', '--both', '--keep', ROOT / 'main.omg'], (ROOT / 'data' / f'{name}.in').read_bytes())
    compare(name, outcome.stdout.decode(), outcome.returncode)
    expected_code = 64 if name.startswith('invalid_') else 0
    assert f'interp exit: {expected_code}' in outcome.stderr.decode(), outcome.stderr.decode()
# Negative control separately: only its exact requires violation counts.
negative = record('intended_invalid', [COMPILER, '--check', '--target', 'linux_x86_64', ROOT / 'intended_invalid/main.omg'])
assert negative.returncode == 1, negative.stderr.decode()
assert '`interval_height` violates required fact `upper > lower`' in negative.stderr.decode(), negative.stderr.decode()
assert 'the instantiated fact is structurally false' in negative.stderr.decode(), negative.stderr.decode()
verify_frozen()
metadata = dict(base=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPOSITORY).decode().strip(), compiler=str(COMPILER), compiler_sha256=hashlib.sha256(COMPILER.read_bytes()).hexdigest(), library_root=ENVIRONMENT['OMEGA_LIBRARY_ROOT'], target='linux_x86_64', frozen_manifest_sha256=hashlib.sha256((ROOT / 'data/frozen.sha256.json').read_bytes()).hexdigest(), executable=str(executable), executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(), validated_native_inputs=29, terminal_cases=8, terminal_scope='CLI compares termination only; emitted native artifacts checked independently', acceptance='passed')
(ARTIFACTS / 'receipt.json').write_text(json.dumps(metadata, indent=2) + '\n')
print('ADD02 acceptance passed: check, native artifacts, Terminal termination, intended rejection', flush=True)
