"""Compare native outputs with pre-frozen bytes; run one child at a time."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('--executable', required=True)
parser.add_argument('--output', default=str(root / 'build' / 'validation.json'))
options = parser.parse_args()
manifest_paths = [root / 'data/oracles.json', root / 'data/supplemental-oracles.json']
manifests = [json.loads(path.read_text()) for path in manifest_paths]
assert all(manifest['integer_tolerance'] == 0 for manifest in manifests)
results = []
for case in [case for manifest in manifests for case in manifest['cases']]:
    name = case['name']
    input_bytes = (root / f'data/{name}.stdin').read_bytes()
    expected = (root / f'data/{name}.stdout').read_bytes()
    assert hashlib.sha256(input_bytes).hexdigest() == case['stdin_sha256']
    assert hashlib.sha256(expected).hexdigest() == case['stdout_sha256']
    completed = subprocess.run([options.executable], input=input_bytes, capture_output=True, timeout=30)
    valid = completed.returncode == case['exit'] and completed.stdout == expected and completed.stderr == b''
    results.append({'name': name, 'expected_exit': case['exit'], 'exit': completed.returncode,
                    'passed': valid, 'stdout': completed.stdout.decode(errors='replace'),
                    'stderr': completed.stderr.decode(errors='replace')})
    print(f"{'PASS' if valid else 'FAIL'} {name}", flush=True)
output = Path(options.output)
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps({'oracle_sha256': hashlib.sha256((root/'data/oracles.json').read_bytes()).hexdigest(),
                              'oracle_hashes': {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in manifest_paths},
                              'executable_sha256': hashlib.sha256(Path(options.executable).read_bytes()).hexdigest(),
                              'passed': sum(item['passed'] for item in results), 'total': len(results),
                              'results': results}, indent=2)+'\n')
raise SystemExit(0 if all(item['passed'] for item in results) else 1)
