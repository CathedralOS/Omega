"""Qualify frozen allocation heuristics, then compare controlled cold/warm runs."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / 'build/unplaced-batch-experiment/qualification'
DATA.mkdir(exist_ok=True)
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'logs'
first.EVIDENCE.mkdir(exist_ok=True)
first.self_test()
producers = json.loads((DATA.parent / 'producer.json').read_text(encoding='utf-8'))['producers']
subjects = dict(first.SUBJECTS)
subjects.update({
    'spill': 'samples/cli/facts/live_across_call_spill/local/main.omg',
    'callee': 'samples/cli/facts/callee_saved_across_nested_calls/local/main.omg',
    'division': 'samples/cli/facts/unsigned_division_remainder/other_machine/main.omg',
    'abi-field': 'samples/cli/facts/abi_multi_arg_spill/field/main.omg',
    'stack16': 'samples/cli/facts/sixteen_arg_stack_calling_plan/other_machine/main.omg',
    'stack-slots': 'samples/cli/facts/stack_frame_slot_isolation/local/main.omg',
})
sources = {}
for relative in subjects.values():
    path = ROOT / relative
    assert (path.parent / 'invalid/main.omg').is_file(), path
    for source in path.parent.rglob('*.omg'):
        sources[str(source.relative_to(ROOT))] = first.digest(source)
records = {'producers': producers, 'sources': sources, 'validation': [], 'warmup': [], 'samples': []}
if (DATA / 'results.json').is_file():
    records = json.loads((DATA / 'results.json').read_text(encoding='utf-8'))
    assert records['producers'] == producers
    assert all(sources[relative] == value for relative, value in records['sources'].items())
    records['sources'] = sources


def retain():
    (DATA / 'results.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')


def environment(flavor, label, cold):
    cache = DATA / ('cache-' + label if cold else 'warm-' + flavor)
    cache.mkdir(exist_ok=not cold)
    result = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32')
    result.pop('OMEGA_PHASE_PROFILE', None)
    return result


for flavor, producer in producers.items():
    assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
    for subject, relative in subjects.items():
        source = ROOT / relative
        label = f'both-{flavor}-{subject}'
        if not any(row['flavor'] == flavor and row['subject'] == subject and row['kind'] == 'both' for row in records['validation']):
            observation, text = first.capture([producer['binary'], 'run', '--both', str(source)], label, environment(flavor, label, True))
            observation.update(flavor=flavor, subject=subject, kind='both')
            records['validation'].append(observation)
            retain()
            assert observation['exit'] == 0 and 'native exit 0' in text and 'interp exit: 0' in text, observation
        label = f'reject-{flavor}-{subject}'
        if not any(row['flavor'] == flavor and row['subject'] == subject and row['kind'] == 'reject' for row in records['validation']):
            observation, text = first.capture([producer['binary'], '--check', str(source.parent / 'invalid/main.omg')], label, environment(flavor, label, True))
            observation.update(flavor=flavor, subject=subject, kind='reject', diagnostic_sha256=first.digest(first.EVIDENCE / (label + '.log')))
            records['validation'].append(observation)
            retain()
            assert observation['exit'] == 1 and 'error' in text.lower(), observation
        print('validated', flavor, subject, flush=True)
for subject in subjects:
    rejects = [row['diagnostic_sha256'] for row in records['validation'] if row['subject'] == subject and row['kind'] == 'reject']
    assert len(set(rejects)) == 1, (subject, rejects)


def compile_one(flavor, subject, label, cold):
    if (DATA / label).exists():
        label += '-retry-1'
    output = DATA / label
    output.mkdir()
    observation, text = first.capture([producers[flavor]['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / subjects[subject])], label, environment(flavor, label, cold))
    artifact = first.compiled_artifact(observation, text, output)
    observation.update(flavor=flavor, subject=subject, regime='cold' if cold else 'warm', native_exit=None)
    records['pending_native'] = observation
    retain()
    execution, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
    observation.update(native_exit=execution['exit'], native_observation=execution)
    assert execution['exit'] == 0, observation
    records.pop('pending_native')
    return observation


for cold in (False, True):
    regime = 'cold' if cold else 'warm'
    for flavor in producers:
        for subject in first.SUBJECTS:
            if any(row['flavor'] == flavor and row['subject'] == subject and row['regime'] == regime for row in records['warmup']):
                continue
            records['warmup'].append(compile_one(flavor, subject, f'warmup-{regime}-{flavor}-{subject}', cold))
            retain()
    for round_number in range(6):
        order = ['baseline', 'candidate'] if round_number % 2 == 0 else ['candidate', 'baseline']
        for subject in first.SUBJECTS:
            for flavor in order:
                if any(row['flavor'] == flavor and row['subject'] == subject and row['regime'] == regime and row['round'] == round_number for row in records['samples']):
                    continue
                label = f'{regime}-{round_number}-{subject}-{flavor}'
                row = compile_one(flavor, subject, label, cold)
                row['round'] = round_number
                records['samples'].append(row)
                retain()
                print(label, round(row['wall_ms'], 1), 'ms', flush=True)
for subject in first.SUBJECTS:
    sizes = {}
    for flavor in producers:
        rows = [row for row in records['samples'] + records['warmup'] if row['subject'] == subject and row['flavor'] == flavor]
        assert len({row['artifact_sha256'] for row in rows}) == 1, (subject, flavor)
        sizes[flavor] = rows[0]['artifact_bytes']
    assert sizes['candidate'] <= sizes['baseline'], (subject, sizes)
assert all(first.digest(ROOT / relative) == value for relative, value in sources.items())
assert all(first.require_optimized(row['binary'])['sha256'] == row['sha256'] for row in producers.values())
records['complete'] = True
retain()
