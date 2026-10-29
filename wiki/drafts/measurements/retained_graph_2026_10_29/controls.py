"""Exercise corrupted/oversized private records through fresh source admission."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[4]
BASE = ROOT / 'build/retained-graph-experiment/comparison'
DATA = BASE / 'controls'
DATA.mkdir()
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA
producers = json.loads((BASE / 'producer.json').read_text(encoding='utf-8'))['producers']
qualified = json.loads((BASE / 'qualification/results.json').read_text(encoding='utf-8'))
assert qualified['complete']
expected = next(row['artifact_sha256'] for row in qualified['samples'] if row['subject'] == 'length')
records = []


def retain():
    (DATA / 'results.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')


def compile_one(flavor, label, cache):
    output = DATA / label
    output.mkdir()
    env = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32', OMEGA_PHASE_PROFILE='1')
    observation, text = first.capture([producers[flavor]['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / first.SUBJECTS['length'])], label, env)
    artifact = first.compiled_artifact(observation, text, output)
    assert observation['artifact_sha256'] == expected, observation
    execution, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
    observation.update(native_exit=execution['exit'], native_observation=execution)
    assert execution['exit'] == 0, observation
    return observation, text


for flavor, producer in producers.items():
    assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
    for kind in ('truncated', 'foreign-compiler', 'checksum', 'oversized'):
        label = flavor + '-' + kind
        cache = DATA / (label + '-cache')
        cache.mkdir()
        seed, _ = compile_one(flavor, label + '-seed', cache)
        paths = [path for path in cache.rglob('*.rec') if path.parent.name == 'source-graphs']
        assert len(paths) == 1
        path = paths[0]
        assert path.resolve().is_relative_to(DATA.resolve())
        original_hash = first.digest(path)
        if kind == 'truncated':
            with path.open('r+b') as stream:
                stream.truncate(64)
        elif kind == 'oversized':
            with path.open('r+b') as stream:
                stream.truncate(512 * 1024 * 1024 + 81)
        else:
            with path.open('r+b') as stream:
                position = 8 if kind == 'foreign-compiler' else 40
                stream.seek(position)
                value = stream.read(1)
                stream.seek(position)
                stream.write(bytes([value[0] ^ 1]))
        mutation = {'bytes': path.stat().st_size, 'sha256': first.digest(path), 'original_sha256': original_hash}
        row = {'flavor': flavor, 'kind': kind, 'seed': seed, 'mutation': mutation}
        records.append(row)
        retain()
        recovered, text = compile_one(flavor, label + '-recovered', cache)
        row['recovered'] = recovered
        row['cache_refused'] = 'source.graph.invalid' in text and 'retained source graph ignored:' in text
        retain()
        assert row['cache_refused'], row
        print('cache control passed', label, flush=True)
assert all(first.digest(ROOT / relative) == value for relative, value in qualified['sources'].items())
assert all(first.require_optimized(row['binary'])['sha256'] == row['sha256'] for row in producers.values())
assert len(records) == 8 and all(row['cache_refused'] for row in records)
print('scoped compiler/build Clippy', flush=True)
env = dict(os.environ, CARGO_UNSTABLE_CHECKSUM_FRESHNESS='true', CARGO_BUILD_FINGERPRINT='content')
with (BASE / 'clippy.stdout').open('w', encoding='utf-8') as output, (BASE / 'clippy.stderr').open('w', encoding='utf-8') as errors:
    result = subprocess.run(['mbx', 'clippy', '--release', '-p', 'compiler', '-p', 'omega-build', '--lib', '--target-dir', str(ROOT / 'build/parallelism-target')], cwd=ROOT, env=env, stdout=output, stderr=errors)
if result.returncode:
    print((BASE / 'clippy.stderr').read_text(encoding='utf-8', errors='replace')[-6000:])
    raise SystemExit(result.returncode)
