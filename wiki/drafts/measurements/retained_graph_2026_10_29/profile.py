"""Build the attribution producer and profile the actual retained graph route."""
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / 'build/retained-graph-experiment'
DATA.mkdir()
TARGET = ROOT / 'build/parallelism-target'
environment = dict(os.environ, CARGO_UNSTABLE_CHECKSUM_FRESHNESS='true', CARGO_BUILD_FINGERPRINT='content')
print('build attribution producer', flush=True)
with (DATA / 'build.stdout').open('w', encoding='utf-8') as output, (DATA / 'build.stderr').open('w', encoding='utf-8') as errors:
    result = subprocess.run(['mbx', 'build', '-p', 'omega', '--release', '--locked', '--message-format=json-render-diagnostics', '--target-dir', str(TARGET)], cwd=ROOT, env=environment, stdout=output, stderr=errors)
if result.returncode:
    print((DATA / 'build.stderr').read_text(encoding='utf-8', errors='replace')[-6000:])
    raise SystemExit(result.returncode)
sys.path.insert(0, str(ROOT / 'tools'))
from compiler_profile import record_cargo_build, require_optimized
binary = TARGET / 'release/omega.exe'
receipt = record_cargo_build(binary, (DATA / 'build.stdout').read_text(encoding='utf-8'))
previous = json.loads((ROOT / 'build/unplaced-batch-experiment/producer.json').read_text(encoding='utf-8'))['producers']['candidate']
for flavor, source in [('baseline', Path(previous['binary']).parent), ('profile', binary.parent)]:
    destination = DATA / flavor
    destination.mkdir()
    for name in ('omega.exe', 'omega.exe.compiler-profile.json', 'omega.pdb'):
        shutil.copy2(source / name, destination / name)
producers = {flavor: require_optimized(DATA / flavor / 'omega.exe') for flavor in ('baseline', 'profile')}
assert producers['baseline']['sha256'] == previous['sha256']
producers['baseline']['source_revision'] = previous['source_revision']
producers['baseline']['applicable_workbench_revision'] = '9ebe45062d8d550465a01077d64a965dec65f838'
producers['profile']['source_revision'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
(DATA / 'producer.json').write_text(json.dumps({'producers': producers, 'build_receipt': receipt}, indent=2) + '\n', encoding='utf-8')
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'attribution'
first.EVIDENCE.mkdir()
first.self_test()
cache = DATA / 'profile-cache'
cache.mkdir()
env = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32', OMEGA_PHASE_PROFILE='1')
previous_results = json.loads((ROOT / 'build/unplaced-batch-experiment/qualification/results.json').read_text(encoding='utf-8'))
expected = {subject: next(row['artifact_sha256'] for row in previous_results['samples'] if row['subject'] == subject and row['flavor'] == 'candidate') for subject in first.SUBJECTS}
records = {'producer': producers['profile'], 'samples': []}
for round_number in range(3):
    for subject, relative in first.SUBJECTS.items():
        label = f'{round_number}-{subject}'
        output = first.EVIDENCE / label
        output.mkdir()
        observation, text = first.capture([producers['profile']['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / relative)], label, env)
        artifact = first.compiled_artifact(observation, text, output)
        assert observation['artifact_sha256'] == expected[subject], observation
        observation.update(subject=subject, round=round_number)
        records['samples'].append(observation)
        (DATA / 'attribution.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
        execution, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
        observation.update(native_exit=execution['exit'], native_observation=execution)
        assert execution['exit'] == 0, observation
        observation['spans'] = [(float(m[1]), int(m[2]), m[3]) for line in text.splitlines() if (m := re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)', line))]
        observation['records'] = [(str(path.relative_to(cache)), path.stat().st_size) for path in cache.rglob('*.rec') if path.parent.name == 'source-graphs']
        print(label, round(observation['wall_ms'], 1), 'ms', flush=True)
        for span in observation['spans']:
            if span[2].startswith(('source.graph.load.', 'build.')):
                print(span, flush=True)
        (DATA / 'attribution.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
assert all(first.digest(ROOT / relative) == value for relative, value in previous_results['sources'].items())
records['complete'] = True
(DATA / 'attribution.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
