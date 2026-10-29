"""Exercise root lookup as an untrusted hint and retain bounded private storage."""
import importlib.util
import json
import os
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / 'build/native-queue-landing/lookup-controls'
DATA.mkdir()
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'logs'
first.EVIDENCE.mkdir()
producer = json.loads((DATA.parent / 'candidate/producer.json').read_text(encoding='utf-8'))['producer']
assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
results = {'producer': producer, 'observations': [], 'controls': []}
length = ROOT / first.SUBJECTS['length']
dispatch = ROOT / first.SUBJECTS['dispatch']


def retain():
    (DATA / 'results.json').write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')


def graphs(cache):
    return {p.resolve() for p in cache.rglob('*.rec') if p.parent.name == 'source-graphs'}


def compile_one(source, cache, label):
    cache.mkdir(exist_ok=True)
    output = DATA / label
    output.mkdir()
    env = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32', OMEGA_PHASE_PROFILE='1')
    row, text = first.capture([producer['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(source)], label, env)
    results['observations'].append(row)
    retain()
    artifact = first.compiled_artifact(row, text, output)
    native, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
    row['native'] = native
    assert native['exit'] == 0, row
    retain()
    return row, text


cache = DATA / 'foreign-root-cache'
a, _ = compile_one(length, cache, 'root-a-seed')
a_records = graphs(cache)
assert len(a_records) == 1, a_records
a_record = next(iter(a_records))
b, _ = compile_one(dispatch, cache, 'root-b-seed')
b_records = graphs(cache) - a_records
assert len(b_records) == 1 and a['artifact_sha256'] != b['artifact_sha256']
b_record = next(iter(b_records))
assert a_record.is_relative_to(DATA.resolve())
shutil.copyfile(b_record, a_record)
recovered, text = compile_one(length, cache, 'valid-foreign-root')
assert recovered['artifact_sha256'] == a['artifact_sha256']
assert 'source.graph.loaded' in text
results['controls'].append({'kind': 'valid-foreign-root', 'recovered': recovered, 'a_record': str(a_record), 'b_record': str(b_record)})
retain()

# Alias only a fresh task-owned replay project; authored repository sources stay fixed.
project = DATA / 'project'
project.mkdir()
for name in ('main.omg', 'build.omg'):
    shutil.copyfile(length.parent / name, project / name)
replay = project / 'main.omg'
cache = DATA / 'mutable-source-cache'
seed, _ = compile_one(replay, cache, 'mutable-source-a')
before = graphs(cache)
(project / 'alias').mkdir()
alias, _ = compile_one(project / 'alias/../main.omg', cache, 'canonical-alias')
assert graphs(cache) == before and alias['artifact_sha256'] == seed['artifact_sha256']
results['controls'].append({'kind': 'canonical-alias', 'record_count': len(before), 'observation': alias})
retain()
shutil.copyfile(dispatch, replay)
changed, _ = compile_one(replay, cache, 'mutable-source-b-warm')
clean, _ = compile_one(replay, DATA / 'mutable-source-clean-cache', 'mutable-source-b-clean')
assert changed['artifact_sha256'] == clean['artifact_sha256'] != seed['artifact_sha256']
assert graphs(cache) == before
results['controls'].append({'kind': 'same-root-changed-source', 'warm': changed, 'clean': clean, 'source_sha256': first.digest(replay)})
retain()

for kind in ('count', 'bytes'):
    cache = DATA / ('prune-' + kind + '-cache')
    seed, _ = compile_one(length, cache, 'prune-' + kind + '-seed')
    current = next(iter(graphs(cache)))
    assert current.is_relative_to(DATA.resolve())
    # Force a fresh store so the real production pruning path runs.
    with current.open('r+b') as stream:
        stream.seek(40)
        value = stream.read(1)[0]
        stream.seek(40)
        stream.write(bytes([value ^ 1]))
    for number in range(66 if kind == 'count' else 2):
        stub = current.parent / (format(number, '064x') + '.rec')
        assert stub != current and not stub.exists()
        with stub.open('wb') as stream:
            stream.truncate(1 if kind == 'count' else 270 * 1024 * 1024)
    initial = {'count': len(graphs(cache)), 'bytes': sum(p.stat().st_size for p in graphs(cache))}
    recovered, text = compile_one(length, cache, 'prune-' + kind + '-recovered')
    retained = graphs(cache)
    final = {'count': len(retained), 'bytes': sum(p.stat().st_size for p in retained)}
    assert current in retained and final['count'] <= 64 and final['bytes'] <= 512 * 1024 * 1024
    assert recovered['artifact_sha256'] == seed['artifact_sha256'] and 'source.graph.invalid' in text
    results['controls'].append({'kind': 'prune-' + kind, 'before': initial, 'after': final, 'recovered': recovered})
    retain()
assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
results['complete'] = True
retain()
print('All root lookup and pruning controls passed', flush=True)
