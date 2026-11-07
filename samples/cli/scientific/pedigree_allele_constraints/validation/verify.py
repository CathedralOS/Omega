"""Compare a compiled Omega application with the frozen byte and exit oracles.
No oracle generation occurs here. Runs one executable at a time.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('executable')
parser.add_argument('--output', type=Path, default=ROOT/'build'/'native-results.json')
options = parser.parse_args()
manifest = json.loads((ROOT/'validation'/'frozen.json').read_text())
results = []
for name, case in manifest['cases'].items():
    supplied = (ROOT/'fixtures'/f'{name}.stdin').read_bytes()
    expected = (ROOT/'fixtures'/f'{name}.stdout').read_bytes()
    observed = subprocess.run([options.executable],input=supplied,capture_output=True,timeout=30)
    ok = observed.returncode == case['exit'] and observed.stdout == expected and observed.stderr == b''
    results.append({'case':name,'exit':observed.returncode,'expected_exit':case['exit'],'ok':ok,
                    'stdout':observed.stdout.decode(errors='replace'),'stderr':observed.stderr.decode(errors='replace')})
    if not ok:
        print(json.dumps(results[-1],indent=2))
options.output.parent.mkdir(parents=True,exist_ok=True)
options.output.write_text(json.dumps(results,indent=2)+'\n')
for index,case in enumerate(json.loads((ROOT/'validation'/'tiny.json').read_text())):
    observed = subprocess.run([options.executable],input=case['stdin'].encode(),capture_output=True,timeout=30)
    ok = observed.returncode == case['exit'] and observed.stdout == case['stdout'].encode() and observed.stderr == b''
    results.append({'case':f'tiny_{index}','exit':observed.returncode,'ok':ok})
    if not ok:
        print(f'tiny {index} mismatch: {case} observed={observed.stdout!r} status={observed.returncode}')
options.output.write_text(json.dumps(results,indent=2)+'\n')
print(f'{sum(item["ok"] for item in results)}/{len(results)} frozen cases agree')
raise SystemExit(0 if all(item['ok'] for item in results) else 1)
