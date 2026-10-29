"""Resolve the small-workload timing concern with profiles and ABBA blocks."""
import importlib.util
import json
import os
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[4]
BASE = ROOT / 'build/unplaced-batch-experiment'
DATA = BASE / 'small-followup'
DATA.mkdir()
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA
qualified = json.loads((BASE / 'qualification/results.json').read_text(encoding='utf-8'))
assert qualified['complete']
producers = qualified['producers']
records = {'producers': producers, 'profiles': [], 'samples': []}


def retain():
    (DATA / 'results.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')


def compile_one(flavor, subject, label, profile=False):
    env = dict(os.environ, LOCALAPPDATA=str(BASE / ('qualification/warm-' + flavor)), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32')
    env.pop('OMEGA_PHASE_PROFILE', None)
    if profile:
        env['OMEGA_PHASE_PROFILE'] = '1'
    output = DATA / label
    output.mkdir()
    observation, text = first.capture([producers[flavor]['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / first.SUBJECTS[subject])], label, env)
    artifact = first.compiled_artifact(observation, text, output)
    reference = next(row for row in qualified['samples'] if row['flavor'] == flavor and row['subject'] == subject)
    assert observation['artifact_sha256'] == reference['artifact_sha256']
    observation.update(flavor=flavor, subject=subject)
    if profile:
        observation['spans'] = [(float(m[1]), int(m[2]), m[3]) for line in text.splitlines() if (m := re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)', line))]
        for span in observation['spans']:
            if span[2] == '06.1.d.05_stage_register_allocation' or '.05.pressure.' in span[2] or span[2] == '06.1.d.05.h_bounded_spill':
                print(label, span, flush=True)
    execution, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
    observation.update(native_exit=execution['exit'], native_observation=execution)
    assert execution['exit'] == 0
    print(label, round(observation['wall_ms'], 1), flush=True)
    return observation


for flavor, producer in producers.items():
    assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
    for subject in ('length', 'dispatch'):
        records['profiles'].append(compile_one(flavor, subject, f'profile-{flavor}-{subject}', True))
        retain()
for block in range(4):
    order = ['baseline', 'candidate', 'candidate', 'baseline'] if block % 2 == 0 else ['candidate', 'baseline', 'baseline', 'candidate']
    for subject in ('length', 'dispatch'):
        for ordinal, flavor in enumerate(order):
            row = compile_one(flavor, subject, f'{block}-{subject}-{ordinal}-{flavor}')
            row['block'] = block
            records['samples'].append(row)
            retain()
assert all(first.digest(ROOT / relative) == value for relative, value in qualified['sources'].items())
assert all(first.require_optimized(row['binary'])['sha256'] == row['sha256'] for row in producers.values())
records['complete'] = True
retain()
