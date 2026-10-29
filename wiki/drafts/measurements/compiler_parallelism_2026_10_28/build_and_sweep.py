import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path.cwd()
evidence = root / 'build/parallelism-first'
evidence.mkdir(parents=True, exist_ok=True)
environment = dict(os.environ, CARGO_UNSTABLE_CHECKSUM_FRESHNESS='true', CARGO_BUILD_FINGERPRINT='content')
command = ['mbx', 'build', '-p', 'omega', '--release', '--message-format=json-render-diagnostics', '--target-dir', str(root / 'build/parallelism-target')]
print('Building exact-source release producer', flush=True)
with (evidence / 'cargo-artifacts.jsonl').open('w', encoding='utf-8') as output, (evidence / 'build.stderr').open('w', encoding='utf-8') as errors:
    completed = subprocess.run(command, cwd=root, env=environment, stdout=output, stderr=errors)
if completed.returncode:
    print((evidence / 'build.stderr').read_text(encoding='utf-8', errors='replace')[-8000:])
    raise SystemExit(completed.returncode)
sys.path.insert(0, str(root / 'tools'))
from compiler_profile import require_optimized, record_cargo_build
binary = root / 'build/parallelism-target/release/omega.exe'
receipt = record_cargo_build(binary, (evidence / 'cargo-artifacts.jsonl').read_text(encoding='utf-8'))
profile = require_optimized(binary)
identity = {'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(), 'command': command, 'profile': profile, 'cargo_receipt': receipt, 'checksum_freshness': True, 'content_fingerprints': True}
(evidence / 'producer.json').write_text(json.dumps(identity, indent=2) + '\n', encoding='utf-8')
print('Producer verified: ' + profile['sha256'], flush=True)
subprocess.run([sys.executable, 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py', '--self-test'], check=True)
subprocess.run([sys.executable, 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py', str(binary)], check=True)
