"""Qualify the actual conflict-resolved producer without relabelling old timings."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / 'build/retained-graph-integration'
DATA.mkdir()
target = ROOT / 'build/parallelism-target'
environment = dict(os.environ, CARGO_UNSTABLE_CHECKSUM_FRESHNESS='true', CARGO_BUILD_FINGERPRINT='content')


def step(label, command):
    print(label, flush=True)
    with (DATA / (label + '.stdout')).open('w', encoding='utf-8') as output, (DATA / (label + '.stderr')).open('w', encoding='utf-8') as errors:
        result = subprocess.run(command, cwd=ROOT, env=environment, stdout=output, stderr=errors)
    if result.returncode:
        print((DATA / (label + '.stderr')).read_text(encoding='utf-8', errors='replace')[-6000:])
        raise SystemExit(result.returncode)


step('build', ['mbx', 'build', '--release', '--locked', '-p', 'omega', '--message-format=json-render-diagnostics', '--target-dir', str(target)])
sys.path.insert(0, str(ROOT / 'tools'))
from compiler_profile import record_cargo_build, require_optimized
binary = target / 'release/omega.exe'
receipt = record_cargo_build(binary, (DATA / 'build.stdout').read_text(encoding='utf-8'))
destination = DATA / 'producer'
destination.mkdir()
for name in ('omega.exe', 'omega.exe.compiler-profile.json', 'omega.pdb'):
    shutil.copy2(binary.parent / name, destination / name)
producer = require_optimized(destination / 'omega.exe')
producer['source_revision'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
(DATA / 'producer.json').write_text(json.dumps({'producer': producer, 'build_receipt': receipt}, indent=2) + '\n', encoding='utf-8')
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'observations'
first.EVIDENCE.mkdir()
first.self_test()
subjects = dict(first.SUBJECTS)
subjects.update({
    'spill': 'samples/cli/facts/live_across_call_spill/local/main.omg',
    'callee': 'samples/cli/facts/callee_saved_across_nested_calls/local/main.omg',
    'division': 'samples/cli/facts/unsigned_division_remainder/other_machine/main.omg',
    'abi-field': 'samples/cli/facts/abi_multi_arg_spill/field/main.omg',
    'stack16': 'samples/cli/facts/sixteen_arg_stack_calling_plan/other_machine/main.omg',
    'stack-slots': 'samples/cli/facts/stack_frame_slot_isolation/local/main.omg',
})
sources = {str(path.relative_to(ROOT)): first.digest(path) for relative in subjects.values() for path in (ROOT / relative).parent.rglob('*.omg')}
results = {'producer': producer, 'sources': sources, 'validation': [], 'compiles': [], 'controls': []}


def retain():
    (DATA / 'results.json').write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')


def env(cache, profile=False):
    result = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32')
    result.pop('OMEGA_PHASE_PROFILE', None)
    if profile:
        result['OMEGA_PHASE_PROFILE'] = '1'
    return result


def compile_one(subject, label, cache, profile=False):
    output = first.EVIDENCE / label
    output.mkdir()
    observation, text = first.capture([producer['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / subjects[subject])], label, env(cache, profile))
    artifact = first.compiled_artifact(observation, text, output)
    execution, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
    observation.update(subject=subject, native_exit=execution['exit'], native_observation=execution)
    assert execution['exit'] == 0, observation
    return observation, text


for subject, relative in subjects.items():
    source = ROOT / relative
    cache = DATA / ('validate-' + subject)
    cache.mkdir()
    observation, text = first.capture([producer['binary'], 'run', '--both', str(source)], 'both-' + subject, env(cache))
    observation.update(subject=subject, kind='both')
    results['validation'].append(observation)
    retain()
    assert observation['exit'] == 0 and 'native exit 0' in text and 'interp exit: 0' in text, observation
    rejection = json.loads((source.parent / 'invalid/sample.json').read_text(encoding='utf-8'))['rejection_contains']
    assert rejection
    observation, text = first.capture([producer['binary'], '--check', str(source.parent / 'invalid/main.omg')], 'reject-' + subject, env(cache))
    observation.update(subject=subject, kind='reject', expected=rejection, diagnostic=text)
    results['validation'].append(observation)
    retain()
    assert observation['exit'] == 1 and all(fragment in text for fragment in rejection), observation
    print('validated integrated producer', subject, flush=True)
cache = DATA / 'warm-cache'
cache.mkdir()
expected = {}
for round_number in range(2):
    for subject in first.SUBJECTS:
        observation, _ = compile_one(subject, f'{round_number}-{subject}', cache, True)
        results['compiles'].append(observation)
        if round_number == 0:
            expected[subject] = observation['artifact_sha256']
        assert observation['artifact_sha256'] == expected[subject]
        retain()
for kind in ('truncated', 'foreign-compiler', 'checksum', 'oversized'):
    cache = DATA / ('control-' + kind)
    cache.mkdir()
    seed, _ = compile_one('length', kind + '-seed', cache)
    records = [path for path in cache.rglob('*.rec') if path.parent.name == 'source-graphs']
    assert len(records) == 1
    path = records[0]
    assert path.resolve().is_relative_to(DATA.resolve())
    with path.open('r+b') as stream:
        if kind in ('truncated', 'oversized'):
            stream.truncate(64 if kind == 'truncated' else 512 * 1024 * 1024 + 81)
        else:
            position = 8 if kind == 'foreign-compiler' else 40
            stream.seek(position)
            byte = stream.read(1)
            stream.seek(position)
            stream.write(bytes([byte[0] ^ 1]))
    mutation = {'bytes': path.stat().st_size, 'sha256': first.digest(path)}
    recovered, text = compile_one('length', kind + '-recovered', cache, True)
    assert recovered['artifact_sha256'] == seed['artifact_sha256'] == expected['length']
    assert 'source.graph.invalid' in text and 'retained source graph ignored:' in text
    results['controls'].append({'kind': kind, 'seed': seed, 'mutation': mutation, 'recovered': recovered})
    retain()
    print('integrated cache control', kind, flush=True)
step('clippy', ['mbx', 'clippy', '--release', '-p', 'compiler', '-p', 'omega-build', '--lib', '--target-dir', str(target)])
assert all(first.digest(ROOT / relative) == value for relative, value in sources.items())
assert require_optimized(producer['binary'])['sha256'] == producer['sha256']
results['complete'] = True
retain()
