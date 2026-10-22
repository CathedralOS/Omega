"""Alternating fresh CLI processes, adapted from source_assembly_2026_10_21/measure.py."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
import time

parser = argparse.ArgumentParser()
parser.add_argument('--baseline', type=Path, required=True)
parser.add_argument('--candidate', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--pairs', type=int, default=12)
parser.add_argument('--profile', action='store_true')
parser.add_argument('--source', type=Path, default=Path('samples/cli/facts/length/local/main.omg'))
parser.add_argument('--minimum-stable-seconds', type=float, default=0)
parser.add_argument('--maximum-pairs', type=int, default=80)
options = parser.parse_args()
root = Path.cwd()
sys.path.insert(0, str(root / 'tools'))
from compiler_profile import require_optimized
binaries = {name: path.resolve() for name, path in [('baseline', options.baseline), ('candidate', options.candidate)]}
identities = {name: require_optimized(path) for name, path in binaries.items()}
records = []
environment = dict(os.environ)
environment.pop('OMEGA_PHASE_PROFILE', None)
if options.profile:
    environment['OMEGA_PHASE_PROFILE'] = '1'
source = (root / options.source).resolve()
minima = {name: float('inf') for name in binaries}
last_minimum = {name: time.monotonic() for name in binaries}
converged = False
maximum_pairs = max(options.pairs, options.maximum_pairs) if options.minimum_stable_seconds else options.pairs
for round_number in range(maximum_pairs):
    for name in (['baseline', 'candidate'] if round_number % 2 == 0 else ['candidate', 'baseline']):
        command = [str(binaries[name]), '--check', '--timings', str(source)]
        result = subprocess.run(command, capture_output=True, text=True, env=environment)
        records.append(dict(round=round_number, producer=name, exit=result.returncode,
                            stdout=result.stdout, stderr=result.stderr,
                            load_average=os.getloadavg() if hasattr(os, 'getloadavg') else None))
        if result.returncode:
            raise RuntimeError((command, result.stdout, result.stderr))
        stage_time = next(float(line.split()[0]) for line in result.stderr.splitlines() if 'Stage 05:' in line)
        if stage_time < minima[name]:
            minima[name] = stage_time
            last_minimum[name] = time.monotonic()
    if round_number + 1 >= options.pairs and options.minimum_stable_seconds:
        converged = all(time.monotonic() - observed >= options.minimum_stable_seconds
                        for observed in last_minimum.values())
        if converged:
            break
for name, path in binaries.items():
    assert require_optimized(path) == identities[name], 'Producer changed during measurement'
result = dict(base=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
              patch_sha256=hashlib.sha256(subprocess.check_output(['git', 'diff', '--', 'omega-rust'])).hexdigest(),
              host=platform.platform(), machine=platform.machine(), cpu_count=os.cpu_count(),
              toolchain=subprocess.check_output(['rustc', '--version'], text=True).strip(),
              source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
              profiles=identities, phase_profile=options.profile,
              command=f'omega --check --timings {source.relative_to(root)}',
              minimum_stable_seconds=options.minimum_stable_seconds,
              minima_converged=converged if options.minimum_stable_seconds else None,
              maximum_pairs=maximum_pairs, records=records)
options.output.parent.mkdir(parents=True, exist_ok=True)
options.output.write_text(json.dumps(result, indent=2) + '\n')
for name in binaries:
    for label in ['Stage 05:', 'total elapsed']:
        values = [float(line.split()[0]) for row in records if row['producer'] == name
                  for line in row['stderr'].splitlines() if label in line]
        print(name, label, 'min/median/mean/max', min(values), statistics.median(values), statistics.mean(values), max(values))
