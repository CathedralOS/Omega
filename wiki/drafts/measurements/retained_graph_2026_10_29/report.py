"""Preserve the bounded-read experiment and compute all-run latency/commit stats."""
import gzip
import json
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / 'build/retained-graph-experiment'
HERE = Path(__file__).resolve().parent


def read(relative):
    return json.loads((DATA / relative).read_text(encoding='utf-8'))


qualified = read('comparison/qualification/results.json')
followup = read('comparison/small-followup/results.json')
controls = read('comparison/controls/results.json')
assert qualified['complete'] and followup['complete']
assert len(qualified['samples']) == 72 and len(qualified['validation']) == 36
assert len(followup['samples']) == 48 and len(controls) == 8
assert all(row['cache_refused'] and row['recovered']['native_exit'] == 0 for row in controls)
summary = {}


def summarize(rows):
    assert rows and all(row['native_exit'] == 0 for row in rows)
    return {
        'wall_median_ms': statistics.median(row['wall_ms'] for row in rows),
        'wall_range_ms': [min(row['wall_ms'] for row in rows), max(row['wall_ms'] for row in rows)],
        'cpu_median_ms': statistics.median(sum(row['root_cpu'].values()) for row in rows),
        'private_commit_median_mib': statistics.median(row['peak_job_private_commit_bytes'] for row in rows) / 1048576,
        'private_commit_range_mib': [min(row['peak_job_private_commit_bytes'] for row in rows) / 1048576, max(row['peak_job_private_commit_bytes'] for row in rows) / 1048576],
    }


for regime in ('warm', 'cold', 'followup'):
    summary[regime] = {}
    source = followup['samples'] if regime == 'followup' else qualified['samples']
    for subject in ('length', 'dispatch', 'stack32'):
        values = {}
        for flavor in ('baseline', 'candidate'):
            rows = [row for row in source if row['subject'] == subject and row['flavor'] == flavor and (regime == 'followup' or row['regime'] == regime)]
            assert len(rows) == (8 if regime == 'followup' else 6)
            values[flavor] = summarize(rows)
        values['wall_change_percent'] = 100 * (values['candidate']['wall_median_ms'] / values['baseline']['wall_median_ms'] - 1)
        values['commit_saved_mib'] = values['baseline']['private_commit_median_mib'] - values['candidate']['private_commit_median_mib']
        values['commit_reduction_percent'] = 100 * values['commit_saved_mib'] / values['baseline']['private_commit_median_mib']
        summary[regime][subject] = values
for subject in ('length', 'dispatch', 'stack32'):
    hashes = {row['artifact_sha256'] for row in qualified['samples'] + qualified['warmup'] + followup['samples'] + followup['profiles'] if row['subject'] == subject}
    assert len(hashes) == 1, (subject, hashes)
logs = {}
for pattern in ('*.stderr', '*.stdout', 'attribution/*.log', 'comparison/*.stderr', 'comparison/*.stdout', 'comparison/qualification/logs/*.log', 'comparison/controls/*.log', 'comparison/small-followup/*.log'):
    for path in DATA.glob(pattern):
        logs[str(path.relative_to(DATA))] = path.read_text(encoding='utf-8', errors='replace')
evidence = {'attribution_producer': read('producer.json'), 'attribution': read('attribution.json'), 'producer': read('comparison/producer.json'), 'qualification': qualified, 'controls': controls, 'followup': followup, 'summary': summary, 'logs': logs}
(HERE / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
(HERE / 'evidence.json.gz').write_bytes(gzip.compress(json.dumps(evidence, indent=2).encode('utf-8'), mtime=0))
for regime, subjects in summary.items():
    for subject, values in subjects.items():
        print(regime, subject, 'wall', round(values['baseline']['wall_median_ms'], 1), '->', round(values['candidate']['wall_median_ms'], 1), 'change%', round(values['wall_change_percent'], 1), 'commit', round(values['baseline']['private_commit_median_mib'], 1), '->', round(values['candidate']['private_commit_median_mib'], 1), 'saved MiB', round(values['commit_saved_mib'], 1))
