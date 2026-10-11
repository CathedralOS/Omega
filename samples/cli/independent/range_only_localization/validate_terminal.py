#!/usr/bin/env python3
"""Check all frozen streams through the documented native/Terminal corpus route.

Only an owned transient fixture is created. Source files are copied unchanged;
its Build dependency path is rebased to the same stable std package. The runner
must be the verified corpus binary from the candidate compiler source.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid
from validate import ROOT, frozen_cases

SOURCES = ['main.omg', 'geometry/model.omg', 'input/parser.omg', 'output/report.omg']

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('runner', type=Path)
    parser.add_argument('--repository', type=Path, default=ROOT.parents[3])
    parser.add_argument('--output', type=Path, default=ROOT / 'build/terminal_validation')
    args = parser.parse_args()
    repo = args.repository.resolve()
    runner = args.runner.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    name = 'add09_range_' + uuid.uuid4().hex + '_exit'
    fixture = repo / 'tests/omega/pass/applications' / name
    fixture.mkdir(parents=True, exist_ok=False)
    records = []
    runner_sha256 = hashlib.sha256(runner.read_bytes()).hexdigest()
    source_hashes = {}
    try:
        for relative in SOURCES:
            destination = fixture / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, destination)
            source_hashes[relative] = hashlib.sha256(destination.read_bytes()).hexdigest()
        dependency = os.path.relpath(repo / 'source/library/std', fixture)
        build = (ROOT / 'build.omg').read_text()
        assert '"../../../../source/library/std"' in build
        (fixture / 'build.omg').write_text(build.replace('"../../../../source/library/std"', json.dumps(dependency)))
        (fixture / 'expected_stderr.txt').write_bytes(b'')
        for case, payload, expected in frozen_cases():
            assert hashlib.sha256(expected).hexdigest() == case['stdout_sha256']
            (fixture / 'input.txt').write_bytes(payload)
            (fixture / 'expected_stdout.txt').write_bytes(expected)
            (fixture / 'expected_native_exit.txt').write_text(str(case['exit']) + '\n')
            golden = output / (case['name'] + '.txt')
            command = ['python3', 'tools/corpus_gate.py', '--runner', str(runner), '--both',
                       '--filter', 'pass/applications/' + name, '--golden', str(golden), '--record']
            result = subprocess.run(command, cwd=repo, capture_output=True, timeout=120)
            (output / (case['name'] + '.log')).write_bytes(result.stdout + result.stderr)
            assert result.returncode == 0, (case['name'], result.returncode)
            facts = golden.read_text()
            for fact in [' built ', f" exit:{case['exit']} ", ' stdout:match ', ' stderr:match ',
                         f" interpreter-exit:{case['exit']} ", ' interpreter-stdout:match ',
                         ' interpreter-stderr:match ', ' interpreter:agrees ']:
                assert fact in facts, (case['name'], fact, facts)
            records.append({'case': case['name'], 'exit': case['exit'], 'passed': True,
                            'stdout_sha256': case['stdout_sha256'], 'command': command})
            (output / 'results.json').write_text(json.dumps({'source_sha256': source_hashes,
                'runner_sha256': runner_sha256,
                'cases': records}, indent=2) + '\n')
            print(case['name'], 'native+Terminal PASS', flush=True)
        assert len(records) == 28
        assert hashlib.sha256(runner.read_bytes()).hexdigest() == runner_sha256
    finally:
        shutil.rmtree(fixture)

if __name__ == '__main__':
    main()
