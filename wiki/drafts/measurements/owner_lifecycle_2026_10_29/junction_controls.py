"""Both producers must refuse cache junctions without reading their valid graphs."""
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[4]
BASE = ROOT / 'build/owner-lifecycle'
DATA = BASE / 'capture/junction-controls'
DATA.mkdir()
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'logs'
first.EVIDENCE.mkdir()
producers = {
    'baseline': json.loads((BASE / 'baseline-producer.json').read_text(encoding='utf-8'))['producer'],
    'candidate': json.loads((BASE / 'diagnostic/producer.json').read_text(encoding='utf-8'))['producer'],
}
results = {'producers': producers, 'controls': [], 'observations': []}
script = DATA / 'create-junction.ps1'
script.write_text('param([string]$Link, [string]$Target)\nNew-Item -ItemType Junction -Path $Link -Target $Target -ErrorAction Stop | Out-Null\n', encoding='utf-8')
powershell = Path(os.environ['SYSTEMROOT']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
assert powershell.is_file()
token = uuid.uuid4().hex[:12]
expected = None


def retain():
    (DATA / 'results.json').write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')


def run_one(flavor, cache, label):
    output = DATA / label
    output.mkdir()
    env = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32', OMEGA_PHASE_PROFILE='1')
    row, text = first.capture([producers[flavor]['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / first.SUBJECTS['length'])], label, env)
    results['observations'].append(row)
    retain()
    if row['exit'] == 0:
        artifact = first.compiled_artifact(row, text, output)
        native, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
        row['native'] = native
        assert native['exit'] == 0
        if expected:
            assert row['artifact_sha256'] == expected
    else:
        assert row['exit'] == 1 and re.search(r'cache|custody|retain|reparse|symlink', text, re.I), row
    retain()
    return row, text


for kind in ('leaf', 'ancestor'):
    outcomes = {}
    for flavor, producer in producers.items():
        assert first.require_optimized(producer['binary'])['sha256'] == producer['sha256']
        cache = Path(os.environ['LOCALAPPDATA']) / f'OmegaPerf-ancestry-junction-{token}-{kind}-{flavor}'
        cache.mkdir()
        label = kind + '-' + flavor
        seed, _ = run_one(flavor, cache, label + '-seed')
        assert seed['exit'] == 0
        expected = seed['artifact_sha256'] if expected is None else expected
        assert seed['artifact_sha256'] == expected
        graphs = [p for p in cache.rglob('*.rec') if p.parent.name == 'source-graphs']
        assert len(graphs) == 1
        graph = graphs[0]
        swapped = graph.parent if kind == 'leaf' else cache / 'CathedralOS'
        original = swapped.with_name(swapped.name + '-retained-original')
        assert swapped.resolve().is_relative_to(cache.resolve()) and not original.exists()
        target = DATA / (label + '-outside')
        target.mkdir()
        valid_graph = target / graph.relative_to(swapped)
        valid_graph.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(graph, valid_graph)
        (target / 'canary.txt').write_text('must remain unchanged\n', encoding='utf-8')
        before = {str(p.relative_to(target)): first.digest(p) for p in target.rglob('*') if p.is_file()}
        # Only a freshly created, verified task-owned cache subtree is renamed.
        swapped.rename(original)
        with (DATA / (label + '-junction.stdout')).open('w', encoding='utf-8') as stdout, (DATA / (label + '-junction.stderr')).open('w', encoding='utf-8') as stderr:
            create = subprocess.run([str(powershell), '-NoProfile', '-NonInteractive', '-File', str(script), '-Link', str(swapped), '-Target', str(target)], stdout=stdout, stderr=stderr, check=False)
        assert create.returncode == 0, label
        metadata = swapped.lstat()
        assert metadata.st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT
        assert metadata.st_reparse_tag == 0xA0000003
        assert swapped.resolve() == target.resolve()
        refused, text = run_one(flavor, cache, label + '-junction')
        loaded = bool(re.search(r'[1-9]\d* n\s+source.graph.loaded', text))
        after = {str(p.relative_to(target)): first.digest(p) for p in target.rglob('*') if p.is_file()}
        row = {'kind': kind, 'flavor': flavor, 'cache': str(cache), 'junction': str(swapped), 'target': str(target), 'seed': seed, 'refused': refused, 'graph_loaded': loaded, 'target_before': before, 'target_after': after, 'diagnostic': text}
        results['controls'].append(row)
        retain()
        assert not loaded and before == after, row
        assert re.search(r'ignored|unavailable|error|failed|invalid', text, re.I), row
        outcomes[flavor] = refused['exit']
        print('junction control', kind, flavor, 'exit', refused['exit'], 'graph not loaded; target unchanged', flush=True)
    assert outcomes['baseline'] == outcomes['candidate'], outcomes
results['complete'] = True
retain()
