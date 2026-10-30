"""Diagnostic only: scope complete graph candidates externally, using one producer.

Only private graph records are moved, between processes, inside this experiment.
All other retained cache lanes remain shared across projects within each policy.
No source graph is split and no package lock or acceptance row is copied.
"""
import importlib.util
import json
import os
from pathlib import Path
import subprocess

ROOT = Path.cwd()
DATA = ROOT / 'build/root-graph-policy'
DATA.mkdir()
integration = json.loads((ROOT / 'build/retained-graph-integration/results.json').read_text(encoding='utf-8'))
assert integration['complete']
producer = integration['producer']
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'logs'
first.EVIDENCE.mkdir()
first.self_test()
assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
reference_cache = ROOT / 'build/retained-graph-integration/warm-cache'
references = [p for p in reference_cache.rglob('*.rec') if p.parent.name == 'source-graphs']
assert len(references) == 1
record_relative = references[0].relative_to(reference_cache)
subjects = dict(first.SUBJECTS)
subjects.update({
    'spill': 'samples/cli/facts/live_across_call_spill/local/main.omg',
    'callee': 'samples/cli/facts/callee_saved_across_nested_calls/local/main.omg',
    'stack-slots': 'samples/cli/facts/stack_frame_slot_isolation/local/main.omg',
})
plan = {
    'question': 'Does restoring unrelated roots dominate warm latency, and does a complete per-root graph lose useful cross-project reuse?',
    'producer': producer,
    'record_relative': str(record_relative),
    'policies': ['shared', 'scoped'],
    'protocol': 'Seed three roots, measure six alternating warm pairs each; then add three more roots and measure three alternating six-root passes. Record first visits separately. Compiler processes and native products are real; record swapping is outside timed compilation.',
    'promotion': 'Only a diagnostic. Promote to a native cache-key candidate if substantial repeatable latency savings survive native byte/oracle guards and first-visit tradeoffs. Preserve full graph lineage and all current admission. Real implementation must retain a bounded total cache budget and be remeasured.',
}
(DATA / 'plan.json').write_text(json.dumps(plan, indent=2) + '\n', encoding='utf-8')
records = {'plan': plan, 'source_sha256': {relative: first.digest(ROOT / relative) for relative in subjects.values()}, 'seed': [], 'warm': [], 'six_root': []}
expected = {}
caches = {}
for policy in ('shared', 'scoped'):
    caches[policy] = DATA / (policy + '-cache')
    caches[policy].mkdir()
slots = DATA / 'root-records'
slots.mkdir()


def retain():
    (DATA / 'results.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')


def compile_one(policy, subject, label):
    cache = caches[policy]
    record = cache / record_relative
    slot = slots / (subject + '.rec')
    if policy == 'scoped':
        assert not record.exists()
        if slot.exists():
            assert slot.resolve().is_relative_to(DATA.resolve())
            assert record.parent.resolve().is_relative_to(DATA.resolve())
            slot.replace(record)
    output = DATA / label
    output.mkdir()
    env = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32')
    env.pop('OMEGA_PHASE_PROFILE', None)
    observation, text = first.capture([producer['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / subjects[subject])], label, env)
    artifact = first.compiled_artifact(observation, text, output)
    observation.update(policy=policy, subject=subject, graph_bytes=record.stat().st_size)
    if subject not in expected:
        expected[subject] = observation['artifact_sha256']
    assert observation['artifact_sha256'] == expected[subject], observation
    if policy == 'scoped':
        assert record.resolve().is_relative_to(DATA.resolve())
        record.replace(slot)
        assert sum(p.stat().st_size for p in slots.glob('*.rec')) <= 512 * 1024 * 1024
    execution, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
    observation.update(native_exit=execution['exit'], native_observation=execution)
    assert execution['exit'] == 0, observation
    print(label, round(observation['wall_ms'], 1), 'ms', round(observation['graph_bytes'] / 1048576, 1), 'MiB graph', flush=True)
    return observation


for index, subject in enumerate(first.SUBJECTS):
    for policy in (('shared', 'scoped') if index % 2 == 0 else ('scoped', 'shared')):
        records['seed'].append(compile_one(policy, subject, f'seed-{policy}-{subject}'))
        retain()
for round_number in range(6):
    for subject in first.SUBJECTS:
        for policy in (('shared', 'scoped') if round_number % 2 == 0 else ('scoped', 'shared')):
            row = compile_one(policy, subject, f'warm-{round_number}-{policy}-{subject}')
            row['round'] = round_number
            records['warm'].append(row)
            retain()
for subject in ('spill', 'callee', 'stack-slots'):
    for policy in ('scoped', 'shared'):
        records['seed'].append(compile_one(policy, subject, f'extend-{policy}-{subject}'))
        retain()
for round_number in range(3):
    for subject in subjects:
        for policy in (('scoped', 'shared') if round_number % 2 == 0 else ('shared', 'scoped')):
            row = compile_one(policy, subject, f'six-{round_number}-{policy}-{subject}')
            row['round'] = round_number
            records['six_root'].append(row)
            retain()
assert all(first.digest(ROOT / path) == digest for path, digest in records['source_sha256'].items())
assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
records['complete'] = True
retain()
