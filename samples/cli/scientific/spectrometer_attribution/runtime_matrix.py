#!/usr/bin/env python3
"""Run one compiled Omega program against every independently frozen input."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--driver', required=True)
    parser.add_argument('--terminal-only', action='store_true', help='Probe standalone Terminal-product execution; default requires both engines')
    args = parser.parse_args()
    driver = Path(args.driver).resolve()
    oracle_bytes = (ROOT / 'data/oracle.json').read_bytes()
    oracle_hash = hashlib.sha256(oracle_bytes).hexdigest()
    assert oracle_hash == (ROOT / 'data/oracle.sha256').read_text().split()[0], 'oracle changed'
    oracle = json.loads(oracle_bytes)
    cases = oracle['cases']
    assert len(cases) == 29
    boundary_bytes = (ROOT / 'data/boundary_oracle.json').read_bytes()
    boundary_hash = hashlib.sha256(boundary_bytes).hexdigest()
    assert boundary_hash == (ROOT / 'data/boundary_oracle.sha256').read_text().split()[0], 'boundary oracle changed'
    boundaries = json.loads(boundary_bytes)['cases']
    assert len(boundaries) == 3 and not (cases.keys() & boundaries.keys())
    cases = {**cases, **boundaries}
    for name, case in cases.items():
        assert digest(ROOT / 'data' / f'{name}.input') == case['input_sha256']
        assert digest(ROOT / 'data' / f'{name}.expected') == case['output_sha256']
    evidence = ROOT / ('build/terminal_matrix' if args.terminal_only else 'build/runtime_matrix')
    evidence.mkdir(parents=True, exist_ok=True)
    (ROOT / 'build/runtime-cases.tsv').write_text(''.join(f"{name}\t{case['exit']}\n" for name, case in cases.items()))
    environment = os.environ.copy()
    environment['OMEGA_LIBRARY_ROOT'] = str(ROOT.parents[3] / 'source/library')
    with (evidence / 'records.txt').open('wb') as output, (evidence / 'stderr.txt').open('wb') as errors:
        command = [str(driver), str(ROOT)] + (['--terminal-only'] if args.terminal_only else [])
        result = subprocess.run(command, env=environment, stdout=output, stderr=errors, timeout=600)
    assert result.returncode == 0, (evidence / 'stderr.txt').read_text()
    assert (evidence / 'check.txt').read_text() == 'Check product succeeded\n'
    results = {}
    for name, case in cases.items():
        results[name] = {}
        for engine in (('Terminal',) if args.terminal_only else ('native', 'Terminal')):
            stdout = evidence / name / f'{engine}.stdout'
            stderr = evidence / name / f'{engine}.stderr'
            exit_code = int((evidence / name / f'{engine}.exit').read_text())
            assert exit_code == case['exit'], (name, engine, exit_code)
            assert stdout.read_bytes() == (ROOT / 'data' / f'{name}.expected').read_bytes(), (name, engine)
            assert stderr.read_bytes() == b'', (name, engine)
            results[name][engine] = dict(exit=exit_code, stdout_sha256=digest(stdout), stderr_sha256=digest(stderr))
    summary = dict(cases=len(cases), executions=len(cases) if args.terminal_only else 2 * len(cases), check='passed', target='linux_x86_64',
                   driver=str(driver), driver_sha256=digest(driver), oracle_sha256=oracle_hash,
                   boundary_oracle_sha256=boundary_hash,
                   native_sha256=None if args.terminal_only else digest(evidence / 'spectrometer.native'),
                   terminal_sha256=digest(evidence / 'program.terminal'),
                   source_sha256={str(path.relative_to(ROOT)): digest(path) for path in sorted(ROOT.rglob('*.omg')) if 'build' not in path.relative_to(ROOT).parts},
                   results=results)
    (evidence / 'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    print(json.dumps({key: value for key, value in summary.items() if key not in ('results', 'source_sha256')}, indent=2))


if __name__ == '__main__':
    main()
