"""Stage unchanged application sources as serial native/Terminal corpus inputs.

Uses the repository's reviewed fixture policy and stream oracles. The temporary
fixture tree is removed even on failure; output evidence stays in build/.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('runner', help='verified corpus_runner binary')
arguments = parser.parse_args()
sample = Path(__file__).resolve().parent
repository = sample.parents[3]
expected = sample / 'data/expected.json'
digest = hashlib.sha256(expected.read_bytes()).hexdigest()
assert digest == (sample / 'data/freeze.sha256').read_text().split()[0]
fixtures = repository / 'tests/omega/run/new079_pump_matrix'
assert not fixtures.exists(), 'refuse to replace an existing fixture tree'
evidence = sample / 'build/terminal-matrix'
evidence.mkdir(parents=True, exist_ok=True)
try:
    for case in json.loads(expected.read_text())['cases']:
        directory = fixtures / case['name']
        directory.mkdir(parents=True)
        shutil.copy2(sample / 'main.omg', directory / 'main.omg')
        for module in ('domain', 'platform'):
            shutil.copytree(sample / module, directory / module)
        (directory / 'build.omg').write_text(
            'machine build(builder: &mut Build) {\n'
            ' Build::application(builder, "new079_pump_matrix");\n'
            ' Build::depend(builder, Source::Path { location: "../../../../../source/library/std" });\n'
            ' Build::bind(builder, linux_x86_64::ProgramEntry, main);\n}\n')
        (directory / 'input.txt').write_text(case['input'])
        (directory / 'expected_stdout.txt').write_text(case['stdout'])
        (directory / 'expected_stderr.txt').write_text('')
        (directory / 'expected_native_exit.txt').write_text(str(case['exit']) + '\n')
    environment = dict(os.environ, OMEGA_CORPUS_JOBS='1')
    with (evidence / 'gate.stdout').open('w') as stdout, (evidence / 'gate.stderr').open('w') as stderr:
        run = subprocess.run([
            'python3', 'tools/corpus_gate.py', '--runner', str(Path(arguments.runner).resolve()),
            '--both', '--golden', 'tests/omega/corpus_outcomes.txt',
            '--filter', 'run/new079_pump_matrix/', '--json', str(evidence / 'verdict.json'),
            '--write-moves', str(evidence / 'moves.json'),
        ], cwd=repository, env=environment, stdout=stdout, stderr=stderr)
    verdict = json.loads((evidence / 'verdict.json').read_text())
    cases = json.loads(expected.read_text())['cases']
    roster = {'run/new079_pump_matrix/' + case['name']: case for case in cases}
    observations = verdict.get('observations', [])
    assert len(observations) == len(roster) == 76, 'incomplete matrix'
    assert {row['fixture'] for row in observations} == set(roster), 'wrong case roster'
    assert not verdict.get('oracle_failures'), 'authored oracle failure'
    for row in observations:
        facts = row.get('facts', {})
        assert row['status'] == 'built', row
        for prefix in ('', 'interpreter-'):
            assert facts.get(prefix + 'exit') == str(roster[row['fixture']]['exit']), row
            assert facts.get(prefix + 'stdout') == 'match', row
            assert facts.get(prefix + 'stderr') == 'match', row
    (evidence / 'oracle.json').write_text(json.dumps({'oracle_sha256': digest, 'gate_exit': run.returncode}, indent=2) + '\n')
    print((evidence / 'gate.stdout').read_text())
    print((evidence / 'gate.stderr').read_text())
    # New scratch fixture records are visible structural moves (exit 1).
    # Authored stream/exit failures are nonzero regressions (exit 3).
    raise SystemExit(0 if run.returncode in (0, 1) else run.returncode)
finally:
    shutil.rmtree(fixtures, ignore_errors=True)
