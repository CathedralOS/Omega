#!/usr/bin/env python3
"""Validate actual compiler/native/Terminal behavior, without changing oracles."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent
REPOSITORY = next(parent for parent in ROOT.parents if (parent / 'AGENTS.md').exists())


def run(command, *, input=None, timeout=600):
    return subprocess.run(command, input=input, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, timeout=timeout, cwd=REPOSITORY)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', required=True)
    parser.add_argument('--native', help='Already published native executable; skips repeated Omega compilation')
    arguments = parser.parse_args()
    omega = str(Path(arguments.omega).resolve())
    build = ROOT / 'build' / 'validation'
    build.mkdir(parents=True, exist_ok=True)
    oracle_bytes = (ROOT / 'data/oracle.json').read_bytes()
    digest = hashlib.sha256(oracle_bytes).hexdigest()
    assert digest == (ROOT / 'data/oracle.sha256').read_text().split()[0], 'oracle changed'
    oracle = json.loads(oracle_bytes)
    boundary_bytes = (ROOT / 'data/boundary_oracle.json').read_bytes()
    boundary_digest = hashlib.sha256(boundary_bytes).hexdigest()
    assert boundary_digest == (ROOT / 'data/boundary_oracle.sha256').read_text().split()[0], 'boundary oracle changed'
    boundaries = json.loads(boundary_bytes)['cases']
    assert len(boundaries) == 3 and not (oracle['cases'].keys() & boundaries.keys())
    oracle['cases'] = {**oracle['cases'], **boundaries}
    evidence = dict(base=run(['git', 'rev-parse', 'HEAD']).stdout.decode().strip(),
                    binary=omega, binary_sha256=hashlib.sha256(Path(omega).read_bytes()).hexdigest(),
                    target='linux_x86_64', oracle_sha256=digest, cases={})
    command = [omega, '--check', '--target', 'linux_x86_64', str(ROOT / 'main.omg')]
    checked = run(command)
    (build / 'check.stdout').write_bytes(checked.stdout)
    (build / 'check.stderr').write_bytes(checked.stderr)
    assert checked.returncode == 0, checked.stderr.decode(errors='replace')
    evidence['check'] = dict(command=command, exit=checked.returncode)
    for name, expected in oracle['cases'].items():
        stdin = (ROOT / 'data' / f'{name}.input').read_bytes()
        stdout = (ROOT / 'data' / f'{name}.expected').read_bytes()
        assert hashlib.sha256(stdin).hexdigest() == expected['input_sha256']
        assert hashlib.sha256(stdout).hexdigest() == expected['output_sha256']
        command = [arguments.native] if arguments.native else [omega, 'run', '--both', '--keep', str(ROOT / 'main.omg')]
        result = run(command, input=stdin)
        (build / f'{name}.stdout').write_bytes(result.stdout)
        (build / f'{name}.stderr').write_bytes(result.stderr)
        record = dict(command=command, exit=result.returncode, stdout_sha256=hashlib.sha256(result.stdout).hexdigest(),
                      stdout_match=result.stdout == stdout)
        evidence['cases'][name] = record
        (build / 'results.json').write_text(json.dumps(evidence, indent=2)+'\n')
        assert result.returncode == expected['exit'], (name, result.returncode, result.stderr.decode(errors='replace'))
        assert result.stdout == stdout, (name, result.stdout.decode(errors='replace'), stdout.decode())
        if arguments.native:
            assert result.stderr == b'', (name, result.stderr)
        else:
            assert f"interp exit: {expected['exit']}".encode() in result.stderr, (name, 'Terminal did not complete', result.stderr)
        print(f'{name}: exit {result.returncode}, output matches', flush=True)
    command = [omega, '--check', '--target', 'linux_x86_64', str(ROOT / 'intended_invalid/main.omg')]
    invalid = run(command)
    (build / 'intended_invalid.stdout').write_bytes(invalid.stdout)
    (build / 'intended_invalid.stderr').write_bytes(invalid.stderr)
    text = (invalid.stdout + invalid.stderr).decode(errors='replace')
    # The package name contains "overflow"; it is not rejection evidence.
    assert invalid.returncode == 1 and re.search(
        r'error:.*(?:may overflow `i32`|not representable.*i32|2147483648)', text, re.I), text
    assert (ROOT / 'intended_invalid/expected.txt').read_text().strip() in text, text
    evidence['intended_invalid'] = dict(command=command, exit=invalid.returncode, diagnostic=text)
    (build / 'results.json').write_text(json.dumps(evidence, indent=2)+'\n')
    print(f"Verified {len(oracle['cases'])} executions and separate intended source rejection")

if __name__ == '__main__':
    main()
