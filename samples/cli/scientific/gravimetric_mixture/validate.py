#!/usr/bin/env python3
"""Run sequential check, native/Terminal replay, and an independent invalid control."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from decimal import Decimal
from fractions import Fraction
from reference import reference, outputs

parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
parser.add_argument('--reference-only', action='store_true')
options = parser.parse_args()
project = Path(__file__).resolve().parent
assert hashlib.sha256((project/'data/expected.json').read_bytes()).hexdigest() == (project/'data/expected.sha256').read_text().split()[0], 'frozen oracle changed'
fixtures = json.loads((project/'data/expected.json').read_text())
for fixture in fixtures['cases']:
    if 'rational' not in fixture:
        continue
    text = fixture['input']
    values = reference(int(text[1:4]), int(text[5:8]), Fraction(int(text[9:12])),
                       Fraction(1) if text[0]=='g' else Fraction(1,1000), text[12]=='1')
    assert {key: str(value) for key,value in values.items()} == fixture['rational'], fixture['name']
    assert outputs(values) == fixture['expected_numbers'], fixture['name']
if options.reference_only:
    print('20 frozen fixtures: independent reference agrees')
    raise SystemExit(0)
log = project/'build/validation'
log.mkdir(parents=True, exist_ok=True)
records = []
def invoke(name, arguments, data=b''):
    command = [options.omega] + arguments
    run = subprocess.run(command, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=240)
    (log/(name+'.stdout')).write_bytes(run.stdout)
    (log/(name+'.stderr')).write_bytes(run.stderr)
    records.append({'name':name,'command':command,'exit':run.returncode,
                    'stdout_sha256':hashlib.sha256(run.stdout).hexdigest(),
                    'stderr_sha256':hashlib.sha256(run.stderr).hexdigest()})
    (log/'runs.json').write_text(json.dumps(records, indent=2)+'\n')
    return run
check = invoke('check', ['--check', str(project/'main.omg')])
assert check.returncode == 0, check.stderr.decode(errors='replace')
domain = invoke('domain_oracles', ['run','--both','--keep',str(project/'verify_domain.omg')])
assert domain.returncode == 0, domain.stderr.decode(errors='replace')
assert 'VERIFIED: domain_oracles' in domain.stdout.decode().splitlines(), domain.stdout.decode()
domain_cases = [case['name'] for case in fixtures['cases']
                if re.fullmatch(r'([gm])[0-9]{3}\1[0-9]{3}\1[0-9]{3}[12]\n', case['input'])]
assert [line.removeprefix('domain case: ') for line in domain.stdout.decode().splitlines()
        if line.startswith('domain case: ')] == domain_cases
assert 'interp exit: 0' in domain.stderr.decode().splitlines()
assert 'declin' not in domain.stderr.decode().lower() and 'divergence' not in domain.stderr.decode().lower()
for fixture in fixtures['cases']:
    # --both executes native then independently replays exactly produced Terminal.
    # CLI comparison only promises status, so also check authored stdout numbers.
    run = invoke(fixture['name'], ['run','--both','--keep',str(project/'main.omg')], fixture['input'].encode())
    text = run.stdout.decode()
    assert run.returncode == fixture['exit'], (fixture['name'], run.returncode, run.stderr.decode(errors='replace'))
    assert fixture['decision'] in text.splitlines(), (fixture['name'], text)
    actual = dict(re.findall(r'^([a-z0-9_]+)=(-?\d+)$', text, re.M))
    expected = fixture.get('expected_numbers', {})
    assert set(actual) == set(expected), (fixture['name'], actual)
    for key, value in expected.items():
        assert abs(Decimal(actual[key])-Decimal(value)) <= fixture['display_tolerance'], (fixture['name'],key,actual[key],value)
    # Decline and disagreement must never pass silently through status matching.
    diagnostics = run.stderr.decode(errors='replace')
    assert f"interp exit: {fixture['exit']}" in diagnostics.splitlines(), (fixture['name'], diagnostics)
    assert 'declin' not in diagnostics.lower() and 'divergence' not in diagnostics.lower(), diagnostics
    print(fixture['name'], 'verified', flush=True)
invalid = invoke('invalid_source', ['--check',str(project/'invalid/main.omg')])
assert invalid.returncode != 0, 'intended-invalid source was accepted'
assert 'cannot prove initializer of `corrected` in Main::main is in domain `i32::Nonnegative`; an annotation cannot establish routed qualification' in invalid.stderr.decode(errors='replace'), invalid.stderr.decode(errors='replace')
print('20 runtime fixtures, 13 native/Terminal domain oracle cases, check and intended-invalid source verified')
