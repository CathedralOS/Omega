"""Run distinct check/native/Terminal/rejection routes, one Omega at a time.

Retain commands, exits, timings, stdout/stderr and complete input sources even
when admission or a compiler boundary fails. No automatic review acceptance.
"""
from pathlib import Path
import argparse
import json
import shutil
import subprocess
import time
import sys

parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
parser.add_argument('--out', required=True)
arguments = parser.parse_args()
root = Path(__file__).resolve().parent
repository = root.parents[4]
output = Path(arguments.out).resolve()
output.mkdir(parents=True, exist_ok=True)
subprocess.run(['python', str(root / 'prepare.py')], check=True)
results = []
for name in ['decay', 'driven_growth', 'invalid_rhs']:
    source = root / name / 'main.omg'
    snapshot = output / 'sources' / name
    snapshot.mkdir(parents=True, exist_ok=True)
    for filename in ['main.omg', 'build.omg', 'rk4.omg']:
        shutil.copyfile(root / name / filename, snapshot / filename)
    routes = [('check', ['--check'])] if name == 'invalid_rhs' else [
        ('check', ['--check']), ('native', ['run']),
        ('terminal', ['inspect-terminal', '--machine', 'Main::main'])]
    for route, flags in routes:
        command = [str(Path(arguments.omega).resolve()), *flags, str(source)]
        start = time.monotonic()
        process = subprocess.run(command, cwd=repository, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        elapsed = time.monotonic() - start
        log = output / f'{name}-{route}.log'
        log.write_text(process.stdout)
        environment_blocked = 'cache custody ancestry' in process.stdout
        row = {'environment_blocked': environment_blocked,
               'qualified': not environment_blocked and name != 'invalid_rhs' and process.returncode == 0,
               'project': name, 'route': route, 'command': command,
               'exit': process.returncode, 'seconds': elapsed, 'log': str(log)}
        results.append(row)
        print(json.dumps(row), flush=True)
        (output / 'results.json').write_text(json.dumps(results, indent=2)+'\n')

# A nonzero invalid-source exit is only evidence; its diagnostic needs review.
sys.exit(1 if any(row['project'] != 'invalid_rhs' and not row['qualified'] for row in results) else 0)
