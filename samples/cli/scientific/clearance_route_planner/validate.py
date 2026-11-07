"""Check immutable original bytes and actual normally admitted execution.

Package review remains a separate normal command. No expected data, decisions,
or lock files are rewritten here.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

FROZEN_SHA256 = 'f595b43edd2a90a959b0c90a23e84845711224faf0d3e9d551836970947d5897'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', type=Path, required=True)
    parser.add_argument('--runtime-harness', type=Path, required=True)
    options = parser.parse_args()
    root = Path(__file__).resolve().parent
    binary, harness = options.omega.resolve(), options.runtime_harness.resolve()
    assert digest(root/'data/expected.json') == FROZEN_SHA256, 'original frozen bytes changed'
    receipt = json.loads(harness.with_suffix('.build.json').read_text())
    assert receipt['compiler_binary_sha256'] == digest(binary), 'compiler receipt mismatch'
    assert receipt['harness_sha256'] == digest(harness), 'harness receipt mismatch'
    assert receipt['harness_source_sha256'] == digest(root/'validation/runtime.rs'), 'stale observer'
    assert receipt['builder_source_sha256'] == digest(root/'validation/build_runtime.py'), 'stale builder'
    output = root/'build/validation'
    output.mkdir(parents=True, exist_ok=True)
    commands = [
        ('independent-reference', [sys.executable, str(root/'reference.py')], 0, None),
        ('complete-source', [str(binary), '--check', str(root/'main.omg')], 0, None),
        # The nested invalid project's fresh diagnostic state is itself part of
        # the parent source inventory. Observe the admitted product first.
        ('native-and-terminal', [str(harness), str(root)], 0, None),
        ('intended-invalid', [str(binary), '--check', str(root/'invalid/main.omg')], 1,
         (root/'invalid/expected.txt').read_bytes().strip()),
    ]
    results = []
    for name, command, expected_exit, diagnostic in commands:
        run = subprocess.run(command, input=b'', capture_output=True, timeout=600)
        (output/(name+'.stdout')).write_bytes(run.stdout)
        (output/(name+'.stderr')).write_bytes(run.stderr)
        passed = run.returncode == expected_exit and (diagnostic is None or diagnostic in run.stderr)
        results.append({'name': name, 'command': command, 'exit': run.returncode, 'passed': passed})
        if not passed:
            break
    record = {'compiler': receipt, 'original_oracle_sha256': FROZEN_SHA256, 'results': results}
    (output/'results.json').write_text(json.dumps(record, indent=2)+'\n')
    for result in results:
        print(json.dumps(result), flush=True)
    return 0 if len(results) == len(commands) and all(result['passed'] for result in results) else 1


if __name__ == '__main__':
    raise SystemExit(main())
