"""Validate original mathematical oracles, intended refusal, and both runtimes.

Package decisions must already be reviewed. This command never changes them.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', required=True, type=Path)
    parser.add_argument('--runtime-harness', required=True, type=Path)
    options = parser.parse_args()
    root = Path(__file__).resolve().parent
    binary = options.omega.resolve()
    harness = options.runtime_harness.resolve()
    receipt = json.loads(harness.with_suffix('.build.json').read_text())
    assert receipt['compiler_binary_sha256'] == digest(binary), 'compiler receipt mismatch'
    assert receipt['harness_sha256'] == digest(harness), 'harness receipt mismatch'
    assert receipt['harness_source_sha256'] == digest(root/'validation/runtime.rs'), 'stale harness source'
    assert receipt['builder_source_sha256'] == digest(root/'validation/build_runtime.py'), 'stale builder source'
    output = root/'build/validation'
    output.mkdir(parents=True, exist_ok=True)
    commands = [
        ('mathematics', ['python3', str(root/'verify_oracles.py')], 0, None),
        ('invalid-alias', [str(binary), '--check', str(root/'invalid_alias/main.omg')], 1,
         (root/'invalid_alias/expected.txt').read_bytes().strip()),
        ('native-and-terminal', [str(harness), str(root)], 0, None),
    ]
    results = []
    for name, command, expected_exit, diagnostic in commands:
        run = subprocess.run(command, capture_output=True, timeout=1800)
        (output/(name+'.stdout')).write_bytes(run.stdout)
        (output/(name+'.stderr')).write_bytes(run.stderr)
        passed = run.returncode == expected_exit and (diagnostic is None or diagnostic in run.stderr)
        results.append({'name': name, 'command': command, 'exit': run.returncode, 'passed': passed})
        if not passed:
            break
    record = {'compiler': receipt, 'oracle_sha256': digest(root/'ORACLES.md'), 'results': results}
    (output/'results.json').write_text(json.dumps(record, indent=2)+'\n')
    for row in results:
        print(json.dumps(row), flush=True)
    return 0 if len(results) == len(commands) and all(row['passed'] for row in results) else 1


if __name__ == '__main__':
    raise SystemExit(main())
