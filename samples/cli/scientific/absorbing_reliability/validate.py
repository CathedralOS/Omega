#!/usr/bin/env python3
"""Compare one compiled native image against every frozen fixture, sequentially."""
import argparse
import json
from pathlib import Path
import subprocess
ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('executable', type=Path)
options = parser.parse_args()
cases = json.loads((ROOT/'data'/'cases.json').read_text())
assert cases['tolerance'] == 0
output = ROOT/'build'/'validation'
output.mkdir(parents=True, exist_ok=True)
records=[]
for case in cases['cases']:
    name=case['name']
    run=subprocess.run([str(options.executable.resolve())], input=(ROOT/'data'/f'{name}.stdin').read_bytes(), capture_output=True, timeout=30)
    (output/f'{name}.stdout').write_bytes(run.stdout)
    (output/f'{name}.stderr').write_bytes(run.stderr)
    passed=run.returncode==case['exit'] and run.stdout==(ROOT/'data'/f'{name}.stdout').read_bytes() and not run.stderr
    records.append(dict(name=name,exit=run.returncode,passed=passed))
(output/'native.json').write_text(json.dumps(records,indent=2)+'\n')
assert all(record['passed'] for record in records), records
print(f'{len(records)} exact native output and exit checks passed')
