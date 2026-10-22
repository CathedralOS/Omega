"""Alternating whole-CLI comparisons using the existing --timings recorder."""
import hashlib
import json
import os
import platform
import re
import statistics
import sys
import subprocess
import time
from pathlib import Path

ROOT = Path.cwd()
OUTPUT = ROOT / 'build/lowering-landing/latest-main'
BINARIES = {name: ROOT / f'build/lowering-landing/omega-{"final" if name == "baseline" else "latest-main"}' for name in ('baseline', 'candidate')}
SUBJECTS = ['samples/cli/collections/segmented_packet_checksum/main.omg']

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

profiles = {name: json.loads(subprocess.check_output([str(path), '--build-profile'], text=True)) for name, path in BINARIES.items()}
assert all(str(profile['opt_level']) != '0' for profile in profiles.values())
record = dict(base=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
              host=platform.platform(), machine=platform.machine(), cpu=subprocess.check_output(['sysctl', '-n', 'machdep.cpu.brand_string'], text=True).strip(),
              rustc=subprocess.check_output(['rustc', '--version'], text=True).strip(), profiles=profiles,
              binaries={name: dict(path=str(path), sha256=digest(path)) for name, path in BINARIES.items()},
              source_hashes={str(path.relative_to(ROOT)): digest(path) for path in ROOT.glob('source/library/**/*.omg')},
              implementation_patch_sha256=digest(ROOT/'build/lowering-landing/latest-main-integration.patch'), records=[])
if '--resume' in sys.argv:
    previous = json.loads((OUTPUT/'timings.json').read_text())
    assert previous['binaries'] == record['binaries']
    record = previous

for subject in SUBJECTS:
    for path in (ROOT/subject).parent.rglob('*.omg'):
        record['source_hashes'][str(path.relative_to(ROOT))] = digest(path)
    for round_number in range(16):
        for name in (('baseline', 'candidate') if round_number % 2 == 0 else ('candidate', 'baseline')):
            if any(row['subject'] == subject and row['round'] == round_number and row['producer'] == name and row['exit'] == 0 for row in record['records']):
                continue
            command = [str(BINARIES[name]), '--timings', subject]
            started = time.perf_counter()
            result = subprocess.run(command, capture_output=True, text=True, timeout=90)
            observation = dict(subject=subject, round=round_number, producer=name, command=command,
                               elapsed_ms=(time.perf_counter()-started)*1000, exit=result.returncode,
                               stdout=result.stdout, stderr=result.stderr, load_average=os.getloadavg())
            emitted = re.search(r'published native output to (.+)', result.stdout)
            if emitted:
                observation['native_sha256'] = digest(Path(emitted.group(1)))
            record['records'].append(observation)
            (OUTPUT/'timings.json').write_text(json.dumps(record, indent=2)+'\n')
            assert result.returncode == 0, observation
        print(subject, round_number+1, flush=True)
summary = {}
for subject in SUBJECTS:
    summary[subject] = {}
    for name in BINARIES:
        observations = [row for row in record['records'] if row['subject']==subject and row['producer']==name]
        summary[subject][name] = {}
        for label in ('terminal-production/lowering:', 'total elapsed'):
            values = [float(line.split()[0]) for row in observations for line in row['stderr'].splitlines() if label in line]
            assert len(values) == len(observations)
            summary[subject][name][label] = dict(min=min(values), median=statistics.median(values), max=max(values), mean=statistics.mean(values), samples=values)
(OUTPUT/'timing-summary.json').write_text(json.dumps(summary, indent=2)+'\n')
print(json.dumps(summary, indent=2), flush=True)

for subject in SUBJECTS:
    observations = [row for row in record['records'] if row['subject']==subject]
    assert len(observations)==32
    assert len({row['native_sha256'] for row in observations})==1, subject
