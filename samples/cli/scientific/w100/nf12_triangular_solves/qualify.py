#!/usr/bin/env python3
"""Record separate routes with one Omega process at a time."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time

parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
parser.add_argument('--timeout', type=int, default=120)
parser.add_argument('--only', choices=['check', 'native', 'terminal', 'rejection'])
arguments = parser.parse_args()
root = Path(__file__).resolve().parent
repository = root.parents[4]
subprocess.run([sys.executable, str(root / 'prepare.py')], check=True)
evidence = root / 'evidence'
evidence.mkdir(exist_ok=True)
revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repository, text=True).strip()
records = []
for project in ('lower_multiple_rhs', 'upper_transpose_unit', 'invalid_unproved_pivot'):
    prepared = root / '.prepared' / project
    source = str(prepared / 'main.omg')
    routes = {'rejection': ['--check', source]} if project.startswith('invalid_') else {
        'check': ['--check', source],
        'native': ['run', source],
        'terminal': ['run', '--both', source],
    }
    for route, suffix in routes.items():
        if arguments.only and route != arguments.only:
            continue
        command = [str(Path(arguments.omega).resolve()), *suffix]
        started = time.monotonic()
        timed_out = False
        try:
            result = subprocess.run(command, cwd=repository, input=b'', capture_output=True, timeout=arguments.timeout)
            exit_code, stdout, stderr = result.returncode, result.stdout, result.stderr
        except subprocess.TimeoutExpired as error:
            exit_code, stdout, stderr = None, error.stdout or b'', error.stderr or b''
            timed_out = True
        stem = f'{project}-{route}'
        (evidence / f'{stem}.stdout.txt').write_bytes(stdout)
        (evidence / f'{stem}.stderr.txt').write_bytes(stderr)
        text = (stdout + stderr).decode('utf-8', 'replace')
        if timed_out:
            classification = 'timeout-before-verdict'
        elif 'custody' in text.lower():
            classification = 'environment-cache-custody-refusal'
        elif project.startswith('invalid_'):
            classification = 'intended-rejection' if exit_code == 1 and 'cannot prove requires contract for call triangular_checked_pivot' in text and '0.0 != 0.0' in text else 'unqualified-rejection-control'
        elif route == 'check' and exit_code == 0:
            classification = 'checked'
        elif route == 'native' and exit_code == 0 and 'PASS nf12' in text:
            classification = 'native-oracle-matched'
        elif route == 'terminal' and exit_code == 0 and 'PASS nf12' in text and 'interp exit: 0' in text and 'DECLINED' not in text:
            classification = 'terminal-comparison-completed'
        else:
            classification = 'unqualified'
        sources = {}
        if exit_code != 0 or timed_out:
            snapshot = evidence / f'{stem}-source'
            snapshot.mkdir(exist_ok=True)
            for filename in ('main.omg', 'build.omg', 'triangular_solves.omg'):
                shutil.copyfile(prepared / filename, snapshot / (filename + ".txt"))
        for filename in ('main.omg', 'build.omg', 'triangular_solves.omg'):
            sources[filename] = hashlib.sha256((prepared / filename).read_bytes()).hexdigest()
        row = dict(project=project, route=route, command=command, cwd=str(repository), revision=revision,
                   host=platform.platform(), elapsed_seconds=round(time.monotonic()-started,3),
                   exit_code=exit_code, timed_out=timed_out, classification=classification, source_sha256=sources)
        records.append(row)
        print(f'{stem}: {classification}; exit {exit_code}', flush=True)
(evidence / f'routes-{arguments.only or "all"}.json').write_text(json.dumps(records, indent=2)+'\n')
