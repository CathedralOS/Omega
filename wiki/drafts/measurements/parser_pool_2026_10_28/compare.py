"""Matched parser-pool experiment, reusing the qualified first-sweep instruments."""
import gzip
import importlib.util
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = ROOT / 'build/parser-pool-experiment'
DATA = EVIDENCE / 'attempt-2'
DATA.mkdir()
spec = importlib.util.spec_from_file_location('first_sweep', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'observations'
first.EVIDENCE.mkdir(exist_ok=True)
first.self_test()
producers = json.loads((EVIDENCE / 'producers.json').read_text(encoding='utf-8'))
original = json.loads(gzip.decompress((ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/evidence.json.gz').read_bytes()))
expected = {subject: next(row['artifact_sha256'] for row in original['retained_cache']['samples'] if row['subject'] == subject) for subject in first.SUBJECTS}
records = {'producers': producers, 'validation': [], 'warmup': [], 'samples': [], 'sources': original['retained_cache']['source_sha256']}
if not (EVIDENCE / 'thread-probes.json').is_file():
    subprocess.run(['pwsh','-NoProfile','-File',str(Path(__file__).with_name('observe_threads.ps1'))],check=True)
probes = json.loads((EVIDENCE / 'thread-probes.json').read_text(encoding='utf-8-sig'))
assert all(row['exit'] == 0 for row in probes)
assert probes[0]['observed_peak_root_threads'] - probes[1]['observed_peak_root_threads'] >= 30


def environment(flavor, label, cold, workers=32):
    cache = DATA / ('cache-' + label if cold else 'warm-' + flavor)
    cache.mkdir(exist_ok=not cold)
    result = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS=str(workers))
    result.pop('OMEGA_PHASE_PROFILE', None)
    return result


def retain():
    (DATA / 'results.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')


def compile_one(flavor, subject, label, cold):
    directory = DATA / label
    directory.mkdir()
    observation, text = first.capture([producers[flavor]['binary'], '--timings', '--build-dir', str(directory),
                                      '--target', 'windows_x86_64', str(ROOT / first.SUBJECTS[subject])],
                                     label, environment(flavor, label, cold))
    artifact = first.compiled_artifact(observation, text, directory)
    observation.update(flavor=flavor, subject=subject, regime='cold' if cold else 'warm')
    if observation['artifact_sha256'] != expected[subject]:
        raise ValueError('Native bytes changed: ' + observation['log'])
    execution = subprocess.run([str(artifact)], stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
    observation['native_exit'] = execution.returncode
    if execution.returncode != 0:
        raise ValueError('Native oracle failed: ' + observation['log'])
    return observation


for flavor, producer in producers.items():
    assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
    for workers in (1,32):
        for subject, relative in first.SUBJECTS.items():
            source = ROOT / relative
            label = f'validate-{flavor}-{subject}-{workers}'
            observation, text = first.capture([producer['binary'], 'run', '--both', str(source)],
                                             label, environment(flavor, label, True, workers))
            observation.update(flavor=flavor, subject=subject, workers=workers, kind='both')
            records['validation'].append(observation)
            retain()
            if observation['exit'] != 0 or 'native exit 0' not in text or 'interp exit: 0' not in text:
                raise ValueError('Both-engine oracle failed: ' + observation['log'])
            label += '-reject'
            observation, text = first.capture([producer['binary'], '--check', str(source.parent / 'invalid/main.omg')],
                                             label, environment(flavor, label, True, workers))
            observation.update(flavor=flavor, subject=subject, workers=workers, kind='reject', diagnostic_sha256=first.digest(first.EVIDENCE / (label + '.log')))
            reference = next(row for row in original['empty_private_cache']['validation'] if row['subject'] == subject and row['kind'] == 'reject')
            records['validation'].append(observation)
            retain()
            if observation['exit'] != 1 or observation['diagnostic_sha256'] != reference['diagnostic_sha256']:
                raise ValueError('Reject changed: ' + observation['log'])
            print('Validated', flavor, subject, workers, flush=True)

for cold in (True, False):
    regime = 'cold' if cold else 'warm'
    for flavor in producers:
        for subject in first.SUBJECTS:
            label = f'warmup-{regime}-{flavor}-{subject}'
            records['warmup'].append(compile_one(flavor, subject, label, cold))
            retain()
    for round_number in range(5):
        order = ['baseline','candidate'] if round_number % 2 == 0 else ['candidate','baseline']
        for subject in first.SUBJECTS:
            for flavor in order:
                label = f'{regime}-{round_number}-{subject}-{flavor}'
                observation = compile_one(flavor, subject, label, cold)
                observation['round'] = round_number
                records['samples'].append(observation)
                retain()
                print(label, round(observation['wall_ms'],1), 'ms', flush=True)

assert all(first.digest(ROOT / path) == value for path, value in records['sources'].items())
assert all(first.require_optimized(producer['binary'])['sha256'] == producer['sha256'] for producer in producers.values())
records['complete'] = True
retain()
print('Matched comparison completed; producer/source guards passed', flush=True)
