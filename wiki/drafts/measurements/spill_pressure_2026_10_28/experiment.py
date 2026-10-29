"""Build and screen one pressure-batching candidate under a build-slot hold."""
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / 'build/spill-pressure-experiment'
DATA.mkdir(exist_ok=True)
TARGET = str(ROOT / 'build/parallelism-target')
environment = dict(os.environ, CARGO_UNSTABLE_CHECKSUM_FRESHNESS='true', CARGO_BUILD_FINGERPRINT='content')


def step(name, command):
    print(name, flush=True)
    with (DATA / (name + '.stdout')).open('w', encoding='utf-8') as output, (DATA / (name + '.stderr')).open('w', encoding='utf-8') as errors:
        result = subprocess.run(command, cwd=ROOT, env=environment, stdout=output, stderr=errors)
    if result.returncode:
        print((DATA / (name + '.stderr')).read_text(encoding='utf-8', errors='replace')[-6000:])
        raise SystemExit(result.returncode)


step('build', ['mbx', 'build', '-p', 'omega', '--release', '--message-format=json-render-diagnostics', '--target-dir', TARGET])
sys.path.insert(0, str(ROOT / 'tools'))
from compiler_profile import record_cargo_build, require_optimized
binary = Path(TARGET) / 'release/omega.exe'
receipt = record_cargo_build(binary, (DATA / 'build.stdout').read_text(encoding='utf-8'))
candidate = DATA / 'candidate'
candidate.mkdir()
for name in ('omega.exe', 'omega.exe.compiler-profile.json', 'omega.pdb'):
    shutil.copy2(binary.parent / name, candidate / name)
producers = {
    'baseline': require_optimized(ROOT / 'build/parser-pool-experiment/baseline/omega.exe'),
    'candidate': require_optimized(candidate / 'omega.exe'),
}
assert producers['baseline']['sha256'] == '3e4fb39360e6b84e89f57b6cd56ac1b486e9146892694ec529a477613fd78720'
producers['baseline']['source_revision'] = 'bf39a3c901bc8ea3ee60b8b57fdc408a591dbacb'
producers['candidate']['source_revision'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
(DATA / 'producer.json').write_text(json.dumps({'producers': producers, 'build_receipt': receipt}, indent=2) + '\n', encoding='utf-8')
step('pressure-tests', ['mbx', 'nextest', 'run', '--release', '-p', 'selected-instructions-to-register-homes', '-E', 'test(assignment::runtime_spill::recovery::candidate_tests)', '--target-dir', TARGET])

spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'screen'
first.EVIDENCE.mkdir()
first.self_test()
records = {'producers': producers, 'profiles': [], 'warmup': [], 'samples': []}


def retain():
    (DATA / 'screen.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')


def compile_one(flavor, label, profile=False):
    cache = DATA / ('cache-' + flavor)
    cache.mkdir(exist_ok=True)
    output = first.EVIDENCE / label
    output.mkdir()
    env = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32')
    env.pop('OMEGA_PHASE_PROFILE', None)
    if profile:
        env['OMEGA_PHASE_PROFILE'] = '1'
    observation, text = first.capture([producers[flavor]['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / first.SUBJECTS['stack32'])], label, env)
    artifact = first.compiled_artifact(observation, text, output)
    execution = subprocess.run([str(artifact)], stdin=subprocess.DEVNULL, capture_output=True, timeout=20)
    observation.update(flavor=flavor, native_exit=execution.returncode)
    assert execution.returncode == 0, observation
    if profile:
        observation['spans'] = [(float(m[1]), int(m[2]), m[3]) for line in text.splitlines() if (m := re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)', line))]
        for span in observation['spans']:
            if span[2] in ('06.1.d.05_stage_register_allocation', '06.1.d.05.i_batch_reanalyze', '06.1.d.05.h_bounded_spill', '06.1.d.05.pressure.unplaced_victim'):
                print(label, span, flush=True)
    print(label, round(observation['wall_ms'], 1), 'ms', observation['artifact_sha256'], flush=True)
    return observation


for flavor in producers:
    records['warmup'].append(compile_one(flavor, 'cold-' + flavor, True))
    records['profiles'].append(compile_one(flavor, 'profile-' + flavor, True))
    retain()
for round_number in range(4):
    order = ['baseline', 'candidate'] if round_number % 2 == 0 else ['candidate', 'baseline']
    for flavor in order:
        records['samples'].append(compile_one(flavor, f'warm-{round_number}-{flavor}'))
        retain()
records['complete'] = True
retain()
