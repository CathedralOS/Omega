"""Attribute the three synchronous lowering owners with matched private caches."""
import importlib.util, json, os, pathlib, re

ROOT = next(parent for parent in pathlib.Path(__file__).resolve().parents if (parent / 'AGENTS.md').is_file())
RECORD = ROOT / 'wiki/drafts/measurements/proof_parallelism_2026_10_28'
specification = importlib.util.spec_from_file_location('fixed_workload_runner', RECORD / 'run.py')
runner = importlib.util.module_from_spec(specification)
specification.loader.exec_module(runner)

def parse_probe(stderr):
    rows = []
    for line in stderr.splitlines():
        if not line.startswith('RECONCILE\t'):
            continue
        _, operation, machines, elapsed, *metrics = line.split('\t')
        assert operation in {'places', 'residences', 'memberships'}
        rows.append(dict(operation=operation, machines=int(machines), wall_ns=int(elapsed),
                         metrics={key:int(value) for key,value in (metric.split('=') for metric in metrics)}))
    return rows

assert parse_probe('RECONCILE\tplaces\t2\t100\tvisits=3\n')[0] == dict(
    operation='places', machines=2, wall_ns=100, metrics={'visits':3})
assert parse_probe('ordinary compiler output\n') == []

def measure(producer, label, mode, comparison=False):
    os.environ['OMEGA_RECONCILE_PROBE'] = mode
    runner.verify_inputs()
    runner.measure(producer, label, 'off', 0, comparison=comparison, cold_cache=not comparison)
    folder = runner.OUT / label
    result = json.loads((folder / 'result.json').read_text())
    rows = parse_probe((folder / 'stderr.txt').read_text())
    if mode != 'off':
        assert rows, 'missing reconciliation measurements'
        rosters = [[row['machines'] for row in rows if row['operation']==operation]
                   for operation in ['places','residences','memberships']]
        assert rosters[0] and rosters[0] == rosters[1] == rosters[2], rosters
    else:
        assert not rows, 'collection unexpectedly enabled'
    result.update(reconciliation_mode=mode, reconciliation=rows)
    (folder / 'result.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    runner.verify_inputs()
    return result

if __name__ == '__main__':
    cases = [
        ('baseline','recon-baseline-a','off'),
        ('reconcile','recon-timing-a','timing'),
        ('reconcile','recon-off-a','off'),
        ('reconcile','recon-timing-b','timing'),
        ('baseline','recon-baseline-b','off'),
        ('reconcile','recon-off-b','off'),
        ('reconcile','recon-timing-c','timing'),
        ('reconcile','recon-counts','counts'),
    ]
    results = [measure(*case) for case in cases]
    expected = '3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
    assert all(result['image_sha256']==expected for result in results), 'image changed'
    result = measure('reconcile','recon-verified','off',comparison=True)
    folder = runner.OUT / 'recon-verified'
    retained = pathlib.Path(re.search(r'build dir kept: (.+)',(folder/'stderr.txt').read_text())[1].strip())
    native = (folder/'stdout.txt').read_bytes()
    assert native == (retained/'terminal.stdout').read_bytes(), 'Terminal stdout differs'
    (folder/'terminal.stdout').write_bytes(native)
    result.update(terminal_stdout_agrees=True, guest_stdout_sha256=runner.digest(folder/'stdout.txt'),
                  retained_image_sha256=runner.digest(retained/'omega-program.exe'))
    assert result['retained_image_sha256']==expected
    (folder/'result.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print('All native images, authored EOF outcomes and Terminal stdout agree.',flush=True)
