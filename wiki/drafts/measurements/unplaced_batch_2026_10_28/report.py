"""Preserve all measurements and logs; summarize without discarding slow runs."""
import gzip
import json
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / 'build/unplaced-batch-experiment'
HERE = Path(__file__).resolve().parent


def read(relative):
    return json.loads((DATA / relative).read_text(encoding='utf-8'))


qualified = read('qualification/results.json')
followup = read('small-followup/results.json')
screen = read('screen.json')
assert qualified['complete'] and followup['complete'] and screen['complete']
assert len(qualified['samples']) == 72 and len(qualified['validation']) == 36
assert len(followup['samples']) == 32
summary = {}
for regime in ('warm', 'cold'):
    summary[regime] = {}
    for subject in ('length', 'dispatch', 'stack32'):
        values = {}
        for flavor in ('baseline', 'candidate'):
            rows = [row for row in qualified['samples'] if row['regime'] == regime and row['subject'] == subject and row['flavor'] == flavor]
            assert len(rows) == 6 and all(row['native_exit'] == 0 for row in rows)
            values[flavor] = {
                'wall_median_ms': statistics.median(row['wall_ms'] for row in rows),
                'wall_range_ms': [min(row['wall_ms'] for row in rows), max(row['wall_ms'] for row in rows)],
                'root_cpu_median_ms': statistics.median(sum(row['root_cpu'].values()) for row in rows),
                'peak_private_commit_median_bytes': statistics.median(row['peak_job_private_commit_bytes'] for row in rows),
                'artifact_bytes': rows[0]['artifact_bytes'],
            }
        values['median_wall_reduction_percent'] = 100 * (1 - values['candidate']['wall_median_ms'] / values['baseline']['wall_median_ms'])
        values['paired_wall_reductions_percent'] = [100 * (1 - next(row['wall_ms'] for row in qualified['samples'] if row['regime'] == regime and row['subject'] == subject and row['flavor'] == 'candidate' and row['round'] == round_number) / next(row['wall_ms'] for row in qualified['samples'] if row['regime'] == regime and row['subject'] == subject and row['flavor'] == 'baseline' and row['round'] == round_number)) for round_number in range(6)]
        summary[regime][subject] = values
summary['small_followup'] = {}
for subject in ('length', 'dispatch'):
    values = {}
    for flavor in ('baseline', 'candidate'):
        rows = [row for row in followup['samples'] if row['subject'] == subject and row['flavor'] == flavor]
        assert len(rows) == 8 and all(row['native_exit'] == 0 for row in rows)
        values[flavor] = statistics.median(row['wall_ms'] for row in rows)
    values['block_wall_reductions_percent'] = [100 * (1 - statistics.mean(row['wall_ms'] for row in followup['samples'] if row['subject'] == subject and row['flavor'] == 'candidate' and row['block'] == block) / statistics.mean(row['wall_ms'] for row in followup['samples'] if row['subject'] == subject and row['flavor'] == 'baseline' and row['block'] == block)) for block in range(4)]
    summary['small_followup'][subject] = values
(HERE / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
logs = {}
for pattern in ('*.stderr', '*.stdout', 'screen/*.log', 'qualification/logs/*.log', 'native-replay/*.log', 'small-followup/*.log'):
    for path in DATA.glob(pattern):
        logs[str(path.relative_to(DATA))] = path.read_text(encoding='utf-8', errors='replace')
evidence = {'summary': summary, 'producer': read('producer.json'), 'screen': screen, 'qualification': qualified, 'small_followup': followup, 'native_replays': read('native-replay.json'), 'logs': logs}
(HERE / 'evidence.json.gz').write_bytes(gzip.compress(json.dumps(evidence, indent=2).encode('utf-8'), mtime=0))
print(json.dumps(summary, indent=2))
