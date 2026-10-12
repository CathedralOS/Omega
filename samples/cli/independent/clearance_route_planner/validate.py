#!/usr/bin/env python3
"""Serial public-CLI validation. Never changes/reviews package decisions or oracles."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('--omega')
p.add_argument('--executable', help='Exact artifact path reported by the preceding native compilation')
p.add_argument('--terminal-harness', type=Path, help='Linked public Terminal runtime validation binary')
p.add_argument('--route', choices=['native', 'both', 'terminal'], required=True)
a = p.parse_args()
if a.route in ('both', 'terminal') and a.executable:
    p.error('--executable cannot supply interpreter comparison')
if a.route == 'both' and not a.omega:
    p.error('--both requires --omega')
if a.route == 'terminal' and not a.terminal_harness:
    p.error('terminal requires --terminal-harness')
if a.route == 'native' and not (a.executable or a.omega):
    p.error('native requires --executable or --omega')
root = Path(__file__).resolve().parent
frozen = json.loads((root/'data/frozen_oracles.json').read_text())
assert frozen['tolerances'] == {'absolute': 0, 'relative': 0}
assert hashlib.sha256((root/'data/reference.py').read_bytes()).hexdigest() == frozen['reference_sha256']
output = root/'build'/('validation-'+a.route)
output.mkdir(parents=True, exist_ok=True)
results = []
if a.route == 'terminal':
    completed = subprocess.run([str(a.terminal_harness.resolve()), str(root)], capture_output=True, timeout=900)
    (output/'harness.stdout.log').write_bytes(completed.stdout)
    (output/'harness.stderr.log').write_bytes(completed.stderr)
    print(completed.stdout.decode(), end='', flush=True)
    assert completed.returncode == 0, completed.stderr.decode(errors='replace')
for case in frozen['cases']:
    name = case['name']
    raw = (root/'data'/(name+'.stdin')).read_bytes()
    expected = (root/'data'/(name+'.stdout')).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == case['stdin_sha256']
    assert hashlib.sha256(expected).hexdigest() == case['stdout_sha256']
    if a.route == 'terminal':
        observed = root/'build/validation/terminal'
        run = subprocess.CompletedProcess([], int((observed/(name+'.exit.txt')).read_text()),
                                        (observed/(name+'.stdout.txt')).read_bytes(),
                                        (observed/(name+'.stderr.txt')).read_bytes())
        command = ['terminal-harness']
    else:
        command = [a.executable] if a.executable else [a.omega, 'run'] + (['--both'] if a.route == 'both' else []) + [str(root/'main.omg')]
        try:
            run = subprocess.run(command, input=raw, capture_output=True, timeout=300)
        except subprocess.TimeoutExpired:
            results.append(dict(name=name, status='TIMEOUT', command=command))
            break
    (output/(name+'.stdout')).write_bytes(run.stdout)
    (output/(name+'.stderr')).write_bytes(run.stderr)
    stderr = run.stderr.decode(errors='replace')
    outcome = 'PASS' if run.returncode == case['exit'] and run.stdout == expected else 'FAIL'
    if a.route == 'both' and 'DECLINED' in stderr:
        outcome = 'DECLINED' if outcome == 'PASS' else outcome
    row = dict(name=name, status=outcome, actual_exit=run.returncode, expected_exit=case['exit'], stdout_equal=run.stdout == expected, command=command)
    results.append(row)
    print(json.dumps(row), flush=True)
    # Compilation/acceptance failure is shared infrastructure, not 22 distinct cases.
    if run.returncode in (200, 202) or outcome == 'DECLINED':
        break
(output/'results.json').write_text(json.dumps(results, indent=2)+'\n')
raise SystemExit(0 if len(results) == len(frozen['cases']) and all(r['status'] == 'PASS' for r in results) else 1)
