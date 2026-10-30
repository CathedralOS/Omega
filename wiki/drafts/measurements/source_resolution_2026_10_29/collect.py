"""Collect the bounded source-resolution experiment; run from its worktree root."""
import hashlib
import json
from pathlib import Path
import shutil
import statistics
import subprocess

root = Path.cwd()
base = root / 'build/source-resolution'
record = root / 'wiki/drafts/measurements/source_resolution_2026_10_29'


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)


def read(path):
    return json.loads(path.read_text())


def stats(rows):
    result = {}
    for variant in 'AB':
        selected = [row for row in rows if row['variant'] == variant]
        values = [row['wall_ms'] for row in selected]
        memory = [row['peak_job_memory_bytes'] for row in selected]
        result[variant] = dict(wall_ms=values, median_ms=statistics.median(values),
                               range_ms=[min(values), max(values)],
                               peak_job_memory_bytes=memory)
    result['median_reduction_percent'] = 100 * (1 - result['B']['median_ms'] / result['A']['median_ms'])
    result['ranges_overlap'] = result['B']['range_ms'][1] >= result['A']['range_ms'][0]
    return result


assert subprocess.check_output(['git', 'diff', '--', 'omega-rust']) == (base / 'candidate.patch').read_bytes()
metadata = read(base / 'comparison/metadata.json')
for producer in metadata['producers'].values():
    assert hashlib.sha256(Path(producer['path']).read_bytes()).hexdigest() == producer['sha256']
for name in ['diagnose.py', 'diagnostic.patch', 'build.log', 'cache-profile.py',
             'cache-screen.py', 'candidate-chain.py', 'candidate.patch',
             'candidate-build.log', 'quality.py', 'quality.log', 'resume.py',
             'comparison.log', 'warm.py']:
    copy(base / name, record / name)
summary = {'decision': 'retained', 'fixed_base': metadata['revision'],
           'production_base': '925264d541a', 'first_compile': {}, 'warm_compile': {}}
native_count = 0
for group, count in [('screen', 6), ('cache-screen', 6)]:
    rows = read(base / group / 'runs.json')
    assert len(rows) == count
    assert all(r['compiler_exit'] == 0 and r['native_exit'] == 0 for r in rows)
    for key in ['artifact_sha256', 'source_sha256', 'build_sha256']:
        assert len({r[key] for r in rows}) == 1
    copy(base / group / 'runs.json', record / group / 'runs.json')
    for row in rows:
        label = f"{row['index']:02}-{row['variant']}"
        copy(base / group / label / 'output.txt', record / group / (label + '.txt'))
    summary[group] = stats(rows)
    native_count += count
identity = read(base / 'identity/observation/result.json')
assert identity['compiler_exit'] == 0 and identity['native_exit'] == 0
for name in ['diagnose.py', 'diagnostic.patch', 'build.log', 'observation/result.json', 'observation/output.txt']:
    copy(base / 'identity' / name, record / 'identity' / name)
native_count += 1
first = read(base / 'comparison/runs.json')
warm = read(base / 'warm/runs.json')
assert len(first) == 36 and len(warm) == 24
for group, rows in [('comparison', first), ('warm', warm)]:
    assert all(r['exit'] == 0 and r['native_exit'] == 0 for r in rows)
    copy(base / group / 'runs.json', record / group / 'runs.json')
    for row in rows:
        label = f"{row['subject']}-{row['index']:02}-{row['variant']}"
        if group == 'comparison':
            label += '-' + row['kind']
        copy(base / group / label / 'output.txt', record / group / (label + '.txt'))
    native_count += len(rows)
copy(base / 'comparison/metadata.json', record / 'comparison/metadata.json')
for subject, source in metadata['sources'].items():
    assert len({r['artifact_sha256'] for r in first + warm if r['subject'] == subject}) == 1
    for filename, key in [('main.omg', 'sha256'), ('build.omg', 'build_sha256')]:
        path = Path(source['path']).with_name(filename)
        assert hashlib.sha256(path.read_bytes()).hexdigest() == source[key]
        copy(path, record / 'inputs' / subject / filename)
    summary['first_compile'][subject] = stats([r for r in first if r['subject'] == subject and r['kind'] == 'measured'])
    summary['warm_compile'][subject] = stats([r for r in warm if r['subject'] == subject])
quality = read(base / 'quality/runs.json')
assert len(quality) == 24
rejects = [r for r in quality if r['label'].endswith('-invalid')]
assert len(rejects) == 2 and all(r['exit'] != 0 and r['matched'] for r in rejects)
native = [r for r in quality if not r['label'].endswith('-invalid')]
assert len(native) == 22 and all(r['exit'] == 0 and r['native_exit'] == 0 for r in native)
malformed = [r for r in quality if 'mutation' in r]
assert len(malformed) == 8 and all(r['record_refused'] for r in malformed)
copy(base / 'quality/runs.json', record / 'quality/runs.json')
for row in quality:
    copy(base / 'quality' / row['label'] / 'output.txt', record / 'quality' / (row['label'] + '.txt'))
native_count += len(native)
assert native_count == 95
summary.update(native_successes=native_count, static_rejects=len(rejects),
               malformed_record_recoveries=len(malformed), excluded_valid_observations=0)
(record / 'summary.json').write_text(json.dumps(summary, indent=2))
print(json.dumps({k: summary[k] for k in ['first_compile', 'warm_compile', 'native_successes', 'static_rejects']}, indent=2))
