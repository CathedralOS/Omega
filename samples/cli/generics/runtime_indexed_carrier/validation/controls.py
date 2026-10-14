"""Check each retained positive/negative input against its authored oracle."""
import argparse
import concurrent.futures
import hashlib
import json
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True, type=Path)
parser.add_argument('--group', required=True, choices=('pass', 'fail'))
args = parser.parse_args()
root = Path(__file__).resolve().parents[5]
inputs = sorted((root / 'tests/omega' / args.group / 'generics').glob('runtime_index_carrier_*/main.omg'))
assert inputs, 'no controls discovered'

def check(path):
    result = subprocess.run([str(args.omega.resolve()), '--check', str(path)],
                            stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=60)
    if args.group == 'pass':
        assert result.returncode == 0, (str(path), result.stderr)
    else:
        reason = (path.parent / 'expected.txt').read_text().strip()
        assert reason and result.returncode == 1 and reason in result.stderr, (str(path), reason, result.returncode, result.stderr)
    return {'input': str(path.relative_to(root)), 'input_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}

with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    observations = list(pool.map(check, inputs))
print(json.dumps({'producer_sha256': hashlib.sha256(args.omega.read_bytes()).hexdigest(),
                  'group': args.group, 'count': len(observations), 'observations': observations}, indent=2))
