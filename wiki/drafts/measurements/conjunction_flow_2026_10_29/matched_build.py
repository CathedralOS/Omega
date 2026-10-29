import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path.cwd()
output = root / 'build/conjunction-flow'
source = root / 'omega-rust/psi/compilation/checking/src/proof_contracts/arithmetic_domains/call_result_bounds.rs'
candidate = source.read_bytes()
baseline = subprocess.check_output(['git', 'show', 'HEAD:' + source.relative_to(root).as_posix()])
assert candidate != baseline
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == '894c94a2eaef8c2e8c362b7282d017f5d53e5516'
os.environ['MBX_CACHE_DIR'] = str(output / 'recovery-cache')
os.environ['MBX_LEARNED_INCREMENTAL'] = '0'
identity = {'base': '894c94a2eaef8c2e8c362b7282d017f5d53e5516',
            'baseline_source_sha256': hashlib.sha256(baseline).hexdigest(),
            'candidate_source_sha256': hashlib.sha256(candidate).hexdigest(),
            'environment': {key: os.environ[key] for key in ['MBX_CACHE_DIR', 'MBX_LEARNED_INCREMENTAL']},
            'build_command': ['mbx', 'build', '-p', 'omega', '--locked', '--target-dir', 'build/pass-producer']}
try:
    source.write_bytes(baseline)
    with (output / 'build-matched-baseline.log').open('w', encoding='utf-8') as log:
        subprocess.run(identity['build_command'], stdout=log, stderr=subprocess.STDOUT, check=True)
    destination = output / 'baseline-matched/omega.exe'
    destination.parent.mkdir(exist_ok=False)
    shutil.copyfile(root / 'build/pass-producer/debug/omega.exe', destination)
    identity['baseline_producer_sha256'] = hashlib.sha256(destination.read_bytes()).hexdigest()
    identity['candidate_producer_sha256'] = hashlib.sha256((output / 'candidate/omega.exe').read_bytes()).hexdigest()
finally:
    source.write_bytes(candidate)
(output / 'matched-build.json').write_text(json.dumps(identity, indent=2), encoding='utf-8')
snapshot = "ConvertTo-Json -InputObject @(Get-Process rustc,omega,mbx -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,CPU,WorkingSet64)"
before = subprocess.run(['pwsh', '-NoProfile', '-Command', snapshot], capture_output=True, text=True)
(output / 'comparison-processes-before.json').write_text(before.stdout, encoding='utf-8')
with (output / 'matched-comparison.log').open('w', encoding='utf-8') as log:
    subprocess.run([sys.executable, 'wiki/drafts/measurements/checking_simplification_2026_10_28/compare.py',
                    str(destination), str(output / 'candidate/omega.exe'), str(output / 'matched-comparison')],
                   stdout=log, stderr=subprocess.STDOUT, check=True)
after = subprocess.run(['pwsh', '-NoProfile', '-Command', snapshot], capture_output=True, text=True)
(output / 'comparison-processes-after.json').write_text(after.stdout, encoding='utf-8')
print('Matched build and comparison completed.', flush=True)
