"""Execute the actual compiled Omega application against immutable frozen files."""
import argparse, hashlib, json, subprocess
from pathlib import Path
parser = argparse.ArgumentParser()
parser.add_argument('--executable', required=True)
parser.add_argument('--output', default=None)
arguments = parser.parse_args()
root = Path(__file__).resolve().parents[1]
manifest = json.loads((root / 'validation/oracle.json').read_text())
output = Path(arguments.output) if arguments.output else root / 'build/validation'
output.mkdir(parents=True, exist_ok=True)
results = []
for name, oracle in manifest['cases'].items():
    input_bytes = (root / 'data' / f'{name}.stdin').read_bytes()
    expected = (root / 'data' / f'{name}.stdout').read_bytes()
    assert hashlib.sha256(input_bytes).hexdigest() == oracle['input_sha256']
    assert hashlib.sha256(expected).hexdigest() == oracle['stdout_sha256']
    result = subprocess.run([str(Path(arguments.executable).resolve())], input=input_bytes,
                            capture_output=True, timeout=30)
    (output / f'{name}.stdout').write_bytes(result.stdout)
    (output / f'{name}.stderr').write_bytes(result.stderr)
    passed = result.returncode == oracle['exit'] and result.stdout == expected and result.stderr == b''
    results.append({'case': name, 'exit': result.returncode, 'expected_exit': oracle['exit'],
                    'stdout_match': result.stdout == expected, 'stderr_empty': not result.stderr,
                    'passed': passed})
    print(f'{"PASS" if passed else "FAIL"} {name}: exit={result.returncode}, stdout={result.stdout == expected}')
(output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
raise SystemExit(0 if all(result['passed'] for result in results) else 1)
