"""Matched real compiler A/B; no external source-graph swapping."""
import importlib.util
import json
import os
from pathlib import Path
import re
import statistics

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / 'build/root-graph-experiment/comparison'
DATA.mkdir()
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'logs'
first.EVIDENCE.mkdir()
first.self_test()
producers = {
    'baseline': json.loads((ROOT / 'build/retained-graph-integration/producer.json').read_text(encoding='utf-8'))['producer'],
    'candidate': json.loads((DATA.parent / 'producer.json').read_text(encoding='utf-8'))['producer'],
}
for producer in producers.values():
    assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
subjects = dict(first.SUBJECTS)
subjects.update({
    'spill': 'samples/cli/facts/live_across_call_spill/local/main.omg',
    'callee': 'samples/cli/facts/callee_saved_across_nested_calls/local/main.omg',
    'stack-slots': 'samples/cli/facts/stack_frame_slot_isolation/local/main.omg',
})
sources = {str(p.relative_to(ROOT)): first.digest(p) for relative in subjects.values() for p in (ROOT / relative).parent.rglob('*.omg')}
records = {'producers': producers, 'sources': sources, 'seed': [], 'warm': [], 'six_root': [], 'profiles': [],
           'protocol': 'Independent default cache directories, real compiler key selection; six alternating paired three-root passes then four six-root passes. Fresh output and process per invocation; warm OS caches. First visits and profiles separate.'}
expected = {}


def retain():
    (DATA / 'results.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')


def compile_one(flavor, subject, label, profile=False):
    cache = DATA / (flavor + '-cache')
    cache.mkdir(exist_ok=True)
    output = DATA / label
    output.mkdir()
    env = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32')
    env.pop('OMEGA_PHASE_PROFILE', None)
    if profile:
        env['OMEGA_PHASE_PROFILE'] = '1'
    observation, text = first.capture([producers[flavor]['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / subjects[subject])], label, env)
    # Persist the compiler result before launching the native image.
    (DATA / (label + '.compile.json')).write_text(json.dumps(observation, indent=2), encoding='utf-8')
    artifact = first.compiled_artifact(observation, text, output)
    observation.update(flavor=flavor, subject=subject)
    expected.setdefault(subject, observation['artifact_sha256'])
    assert observation['artifact_sha256'] == expected[subject], observation
    execution, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
    observation.update(native_exit=execution['exit'], native_observation=execution)
    assert execution['exit'] == 0, observation
    graphs = [p for p in cache.rglob('*.rec') if p.parent.name == 'source-graphs']
    observation.update(graph_count=len(graphs), graph_bytes=sum(p.stat().st_size for p in graphs))
    assert observation['graph_bytes'] <= 512 * 1024 * 1024
    if profile:
        observation['cache_loaded'] = bool(re.search(r'[1-9]\d* n\s+source.graph.loaded', text))
        observation['prepared_hit'] = bool(re.search(r'[1-9]\d* n\s+source.graph.prepared_hits', text))
        assert observation['cache_loaded'] and observation['prepared_hit'], observation
        observation['spans'] = [(float(m[1]), int(m[2]), m[3]) for line in text.splitlines() if (m := re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)', line))]
    print(label, round(observation['wall_ms'], 1), 'ms', observation['graph_count'], 'graphs', flush=True)
    return observation


retain()
for index, subject in enumerate(first.SUBJECTS):
    for flavor in (('baseline', 'candidate') if index % 2 == 0 else ('candidate', 'baseline')):
        records['seed'].append(compile_one(flavor, subject, 'seed-' + flavor + '-' + subject))
        retain()
for round_number in range(6):
    for subject in first.SUBJECTS:
        for flavor in (('baseline', 'candidate') if round_number % 2 == 0 else ('candidate', 'baseline')):
            row = compile_one(flavor, subject, f'warm-{round_number}-{flavor}-{subject}')
            row['round'] = round_number
            records['warm'].append(row)
            retain()
for index, subject in enumerate(('spill', 'callee', 'stack-slots')):
    for flavor in (('candidate', 'baseline') if index % 2 == 0 else ('baseline', 'candidate')):
        records['seed'].append(compile_one(flavor, subject, 'extend-' + flavor + '-' + subject))
        retain()
for round_number in range(4):
    for subject in subjects:
        for flavor in (('candidate', 'baseline') if round_number % 2 == 0 else ('baseline', 'candidate')):
            row = compile_one(flavor, subject, f'six-{round_number}-{flavor}-{subject}')
            row['round'] = round_number
            records['six_root'].append(row)
            retain()
for subject in first.SUBJECTS:
    for flavor in producers:
        records['profiles'].append(compile_one(flavor, subject, 'profile-' + flavor + '-' + subject, True))
        retain()
assert all(first.digest(ROOT / p) == value for p, value in sources.items())
assert all(first.require_optimized(p['binary'])['sha256'] == p['sha256'] for p in producers.values())
records['complete'] = True
retain()
for cohort in ('warm', 'six_root'):
    for subject in sorted({r['subject'] for r in records[cohort]}):
        medians = {f: statistics.median(r['wall_ms'] for r in records[cohort] if r['subject'] == subject and r['flavor'] == f) for f in producers}
        print(cohort, subject, medians, 'reduction', 1 - medians['candidate'] / medians['baseline'], flush=True)
