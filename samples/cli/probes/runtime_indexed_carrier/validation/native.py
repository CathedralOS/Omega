"""Execute the frozen native customer and check authored process observations."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--executable', required=True, type=Path)
args = parser.parse_args()
observed = subprocess.run([str(args.executable.resolve())], stdin=subprocess.DEVNULL,
                          capture_output=True, timeout=30)
assert observed.returncode == 70, observed.returncode
assert observed.stdout == b'' and observed.stderr == b'', (observed.stdout, observed.stderr)
print(json.dumps({'native_exit': observed.returncode, 'stdout_bytes': 0, 'stderr_bytes': 0,
                  'artifact_sha256': hashlib.sha256(args.executable.read_bytes()).hexdigest()}))
