#!/usr/bin/env python3
"""Check frozen exact exits/stdout. Never regenerates or adapts the oracle."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
route = parser.add_mutually_exclusive_group(required=True)
route.add_argument('--binary', type=Path)
route.add_argument('--omega', type=Path)
parser.add_argument('--both', action='store_true')
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--case', action='append', help='Select named frozen cases; omit to run all')
args = parser.parse_args()
if args.both and args.binary:
    parser.error('--both requires --omega')
for filename, digest in json.loads((ROOT/'data/FIXTURE_SHA256.json').read_text()).items():
    assert hashlib.sha256((ROOT/'data'/filename).read_bytes()).hexdigest() == digest, f'Frozen fixture changed: {filename}'
records = []
fixtures = json.loads((ROOT/'data/cases.json').read_text())
if args.case:
    names = {fixture['name'] for fixture in fixtures}
    assert set(args.case) <= names, 'Unknown frozen case'
    fixtures = [fixture for fixture in fixtures if fixture['name'] in args.case]
for fixture in fixtures:
    command = [str(args.binary.resolve())] if args.binary else [str(args.omega.resolve()),'run'] + (['--both'] if args.both else []) + [str(ROOT/'main.omg')]
    request = (ROOT/'data'/fixture['input']).read_bytes()
    expected = (ROOT/'data'/fixture['stdout']).read_bytes()
    observed = subprocess.run(command,input=request,capture_output=True,timeout=180)
    matches = observed.returncode == fixture['exit'] and observed.stdout == expected
    stderr = observed.stderr.decode(errors='replace')
    # DECLINED is explicitly unsupported, never a successful Terminal check.
    comparison = None if not args.both else ('diverged' if 'DIVERGENCE' in stderr else 'declined' if 'DECLINED' in stderr else 'agreed' if f"interp exit: {fixture['exit']}\n" in stderr else 'unconfirmed')
    passed = matches and (not args.both or comparison == 'agreed')
    record = dict(name=fixture['name'],command=command,exit=observed.returncode,expected_exit=fixture['exit'],stdout_match=observed.stdout==expected,native_passed=matches,passed=passed,comparison=comparison,stdin_sha256=hashlib.sha256(request).hexdigest(),stdout=observed.stdout.decode(errors='replace'),stderr=stderr)
    records.append(record)
    print(f"{fixture['name']}: {'PASS' if passed else 'FAIL'} exit={observed.returncode} comparison={comparison}",flush=True)
    if not passed:
        break  # one unexplained route failure is enough to investigate
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(records,indent=2)+'\n')
raise SystemExit(0 if len(records)==len(fixtures) and all(r['passed'] for r in records) else 1)
