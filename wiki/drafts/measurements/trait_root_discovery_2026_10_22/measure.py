"""Paired compiler-throughput capture using the existing benchmark primitives.

Run from the repository root with frozen omega-baseline and omega-candidate
executables in build/throughput. No compiler build or corpus run should overlap.
The sources here have no authored external dependency and need no review lock.
"""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import random
try:
    import resource
except ImportError:
    resource = None
import statistics
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path('tools/benchmark').resolve()))
from benchmark import capture_compile, parse_timings, require_optimized, run_measured

folder = Path('build/root-discovery').resolve()
subjects = [
    'samples/cli/facts/length/local/main.omg',
    'samples/cli/facts/dyn_dispatch_selector_payload_bound/local/main.omg',
    'samples/cli/facts/nested_nine_level_record_field_bound/local/main.omg',
]
producers = {name: folder / f'omega-{name}' for name in ('baseline', 'candidate')}
identity = {}
for name, path in producers.items():
    require_optimized(path)
    identity[name] = dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        profile=json.loads(subprocess.check_output([str(path), '--build-profile'], text=True)))
records = []
metadata = dict(subject_order_seed=20261021, started_utc=datetime.now(timezone.utc).isoformat(), base=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    patch_sha256=hashlib.sha256(subprocess.check_output(['git', 'diff', '--', 'omega-rust'])).hexdigest(),
    library_tree=subprocess.check_output(['git', 'rev-parse', 'HEAD:source/library'], text=True).strip(),
    rustc=subprocess.check_output(['rustc', '--version'], text=True).strip(),
    host=platform.platform(), producers=identity,
    sources={str(path): hashlib.sha256(path.read_bytes()).hexdigest()
             for subject in subjects for path in Path(subject).parent.glob('*.omg')})

def capture(name, subject, round_number):
    with tempfile.TemporaryDirectory(prefix='omega-throughput-') as output:
        command = [str(producers[name]), '--timings', '--target', 'macos_arm64',
                   '--build-dir', output, str(Path(subject).resolve())]
        before = resource.getrusage(resource.RUSAGE_CHILDREN) if resource else None
        code, wall_ms, rss, text = capture_compile(command, Path.cwd())
        after = resource.getrusage(resource.RUSAGE_CHILDREN) if resource else None
        stages, published = parse_timings(text)
        record = dict(producer=name, subject=subject, round=round_number,
            command=command, exit=code, wall_ms=wall_ms, peak_rss_bytes=rss,
            user_cpu_ms=1000 * (after.ru_utime - before.ru_utime) if resource else None,
            system_cpu_ms=1000 * (after.ru_stime - before.ru_stime) if resource else None,
            timings_ms=stages, output=text, load_average=os.getloadavg() if hasattr(os, 'getloadavg') else None)
        records.append(record)
        (folder/'paired.json').write_text(json.dumps(dict(metadata=metadata, records=records), indent=2)+'\n')
        if code != 0 or published is None:
            raise RuntimeError(f'{name}: {subject}: compile failed: {text}')
        artifact = Path(published)
        record['native_sha256'] = hashlib.sha256(artifact.read_bytes()).hexdigest()
        record['native_bytes'] = artifact.stat().st_size
        runtime_exit, _, _ = run_measured([str(artifact)], Path.cwd())
        record['native_exit'] = runtime_exit
        (folder/'paired.json').write_text(json.dumps(dict(metadata=metadata, records=records), indent=2)+'\n')
        if runtime_exit != 0:
            raise RuntimeError(f'{name}: {subject}: native exit {runtime_exit}')

# An explicit, retained warmup for every producer/subject precedes the pairs.
for subject in subjects:
    for name in producers:
        capture(name, subject, -1)
ordering = random.Random(20261021)
for round_number in range(12):
    round_subjects = list(subjects)
    ordering.shuffle(round_subjects)
    for subject in round_subjects:
        order = ('baseline', 'candidate') if round_number % 2 == 0 else ('candidate', 'baseline')
        for name in order:
            capture(name, subject, round_number)
    print('paired round', round_number + 1, flush=True)
summary = {}
for subject in subjects:
    summary[subject] = {}
    for name in producers:
        rows = [r for r in records if r['subject'] == subject and r['producer'] == name and r['round'] >= 0]
        summary[subject][name] = {metric: dict(median=statistics.median(r[metric] for r in rows),
            minimum=min(r[metric] for r in rows), maximum=max(r[metric] for r in rows))
            for metric in ('wall_ms', 'peak_rss_bytes', 'user_cpu_ms', 'system_cpu_ms')
            if all(r[metric] is not None for r in rows)}
    pairs = [(next(r['wall_ms'] for r in records if r['subject']==subject and r['round']==number and r['producer']=='candidate') -
              next(r['wall_ms'] for r in records if r['subject']==subject and r['round']==number and r['producer']=='baseline'))
             for number in range(12)]
    summary[subject]['paired_candidate_minus_baseline_ms'] = dict(median=statistics.median(pairs), values=pairs)
result = dict(metadata=metadata, records=records, summary=summary)
(folder/'paired.json').write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps(summary, indent=2))
