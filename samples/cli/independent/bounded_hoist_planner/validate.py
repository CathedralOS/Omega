#!/usr/bin/env python3
"""Public-CLI validation. Never changes/reviews package decisions or oracles.

`--route both` compiles once with `omega run --both --inputs data`, which runs
every case natively and replays it through the Terminal reference; `native`
runs an executable or `omega run` per case.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('--omega')
p.add_argument('--executable', help='Exact artifact path reported by the preceding native compilation')
p.add_argument('--route', choices=['native', 'both'], required=True)
a = p.parse_args()
if a.route == 'both' and a.executable:
    p.error('--executable cannot supply interpreter comparison')
if a.route == 'both' and not a.omega:
    p.error('--both requires --omega')
if a.route == 'native' and not (a.executable or a.omega):
    p.error('native requires --executable or --omega')
root = Path(__file__).resolve().parent
frozen = json.loads((root/'data/frozen_oracles.json').read_text())
assert frozen['tolerances'] == {'absolute': 0, 'relative': 0}
assert hashlib.sha256((root/'data/reference.py').read_bytes()).hexdigest() == frozen['reference_sha256']
output = root/'build'/('validation-'+a.route)
output.mkdir(parents=True, exist_ok=True)
results = []
statuses = {}
if a.route == 'both':
    completed = subprocess.run([a.omega, 'run', '--both', '--inputs', str(root/'data'),
                                '--outputs', str(output), str(root/'main.omg')],
                               stdin=subprocess.DEVNULL, capture_output=True, timeout=1800)
    (output/'cli.stderr.log').write_bytes(completed.stderr)
    assert completed.returncode == 0, completed.stderr.decode(errors='replace')
    for line in (output/'results.tsv').read_text().splitlines()[1:]:
        row = line.split('\t')
        statuses[row[0]] = row
for case in frozen['cases']:
    name = case['name']
    raw = (root/'data'/(name+'.stdin')).read_bytes()
    expected = (root/'data'/(name+'.stdout')).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == case['stdin_sha256']
    assert hashlib.sha256(expected).hexdigest() == case['stdout_sha256']
    if a.route == 'both':
        _, native, verdict, interpreter = statuses[name]
        stdout = (output/(name+'.stdout')).read_bytes()
        engines_agree = (verdict == 'agrees' and native == interpreter
                         and (output/(name+'.terminal.stdout')).read_bytes() == stdout
                         and (output/(name+'.terminal.stderr')).read_bytes() == b'')
        run = subprocess.CompletedProcess([], int(native), stdout, (output/(name+'.stderr')).read_bytes())
        command = ['omega run --both --inputs']
        if not engines_agree:
            run.returncode = -1
    else:
        command = [a.executable] if a.executable else [a.omega, 'run', str(root/'main.omg')]
        try:
            run = subprocess.run(command, input=raw, capture_output=True, timeout=300)
        except subprocess.TimeoutExpired:
            results.append(dict(name=name, status='TIMEOUT', command=command))
            break
        (output/(name+'.stdout')).write_bytes(run.stdout)
        (output/(name+'.stderr')).write_bytes(run.stderr)
    stderr = run.stderr.decode(errors='replace')
    # The program writes only stdout. The CLI forwards program stderr before
    # its own fixed exit reports; compare the entire stream so extra errors
    # cannot disappear behind an otherwise correct stdout/exit result.
    expected_stderr = b''
    if a.omega and not a.executable and a.route == 'native':
        expected_stderr = f"native exit {case['exit']}\n".encode()
    stderr_equal = run.stderr == expected_stderr
    outcome = 'PASS' if run.returncode == case['exit'] and run.stdout == expected and stderr_equal else 'FAIL'
    row = dict(name=name, status=outcome, actual_exit=run.returncode, expected_exit=case['exit'], stdout_equal=run.stdout == expected, stderr_equal=stderr_equal, command=command)
    results.append(row)
    print(json.dumps(row), flush=True)
    # Compilation/acceptance failure is shared infrastructure, not 22 distinct cases.
    if run.returncode in (200, 202) or outcome == 'DECLINED':
        break
(output/'results.json').write_text(json.dumps(results, indent=2)+'\n')
raise SystemExit(0 if len(results) == len(frozen['cases']) and all(r['status'] == 'PASS' for r in results) else 1)
