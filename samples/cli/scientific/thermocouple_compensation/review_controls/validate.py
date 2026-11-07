#!/usr/bin/env python3
"""Run separately frozen review boundary controls; original oracle stays intact."""
import argparse, hashlib, importlib.util, json, subprocess, time
from pathlib import Path
HERE = Path(__file__).resolve().parent
APP = HERE.parent
EXPECTED_SHA256 = {'derivative':'f476b7399540b99526c5ad406d8387d03ad140eedb9258b63fe2f758bedb57b2', 'continuity':'d540cd9c4c6b3f14f15c44f9929cb87ca70d514e454ca72b2762d331df23aa01'}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--route', choices=['native', 'both'], required=True)
    parser.add_argument('--program', type=Path)
    parser.add_argument('--omega', type=Path)
    parser.add_argument('--cases', help='Comma-separated subset; default all six')
    args = parser.parse_args()
    selected = args.cases.split(',') if args.cases else None
    if selected is not None: assert len(selected) == len(set(selected))
    all_names = set()
    suites = []
    for suite, folder in [('derivative', HERE), ('continuity', HERE/'continuity')]:
        assert hashlib.sha256((folder/'expected.json').read_bytes()).hexdigest() == EXPECTED_SHA256[suite]
        expected = json.loads((folder/'expected.json').read_text())
        all_names.update(expected['cases'])
        suites.append((suite, folder, expected))
    if selected is not None: assert set(selected) <= all_names
    if args.route == 'native':
        assert args.program is not None
        command = [str(args.program.resolve())]
    else:
        assert args.omega is not None
        command = [str(args.omega.resolve()), 'run', '--both', '--keep', str(APP/'main.omg')]
    for suite, folder, expected in suites:
        output = APP/'build'/'review_validation'/args.route/suite
        output.mkdir(parents=True, exist_ok=True)
        names = [name for name in expected['cases'] if selected is None or name in selected]
        for name, case in expected['cases'].items():
            if name not in names: continue
            data = (folder/'data'/f'{name}.txt').read_bytes()
            assert hashlib.sha256(data).hexdigest() == expected['fixture_sha256'][f'{name}.txt']
            started = time.monotonic()
            result = subprocess.run(command, input=data, capture_output=True, timeout=600)
            for suffix, value in [('stdout', result.stdout), ('stderr', result.stderr)]:
                (output/f'{name}.{suffix}').write_bytes(value)
            (output/f'{name}.exit').write_text(str(result.returncode)+'\n')
            (output/f'{name}.command.json').write_text(json.dumps({'command':command, 'exit':result.returncode, 'seconds':time.monotonic()-started}, indent=2)+'\n')
            if args.route == 'both':
                diagnostics = result.stderr.decode()
                assert f"interp exit: {case['exit']}" in diagnostics and 'DECLINED' not in diagnostics and 'DIVERGENCE' not in diagnostics, diagnostics
    spec = importlib.util.spec_from_file_location('independent_reference', APP/'reference.py')
    reference = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(reference)
    for suite, folder, expected in suites:
        names = [name for name in expected['cases'] if selected is None or name in selected]
        if not names: continue
        reference.ROOT = folder
        reference.verify(APP/'build'/'review_validation'/args.route/suite, names)

if __name__ == '__main__': main()
