#!/usr/bin/env python3
"""Sequential exact output checks against frozen files, retaining each result."""
import argparse,hashlib,json,subprocess
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('--executable',required=True)
parser.add_argument('--artifacts',required=True)
arguments=parser.parse_args()
root=Path(__file__).resolve().parent
artifacts=Path(arguments.artifacts).resolve()
artifacts.mkdir(parents=True,exist_ok=True)
for line in (root/'data/FROZEN.sha256').read_text().splitlines():
    digest,name=line.split('  ',1)
    assert hashlib.sha256((root/'data'/name).read_bytes()).hexdigest()==digest,name
results=[]
for case in json.loads((root/'data/cases.json').read_text()):
    name=case['name']
    result=subprocess.run([str(Path(arguments.executable).resolve())],input=(root/'data'/f'{name}.stdin').read_bytes(),capture_output=True,timeout=30)
    (artifacts/f'{name}.stdout').write_bytes(result.stdout)
    (artifacts/f'{name}.stderr').write_bytes(result.stderr)
    passed=(result.returncode==case['exit'] and result.stdout==(root/'data'/f'{name}.stdout').read_bytes() and not result.stderr)
    results.append(dict(name=name,status=result.returncode,passed=passed))
(artifacts/'results.json').write_text(json.dumps(results,indent=2)+'\n')
failed=[result for result in results if not result['passed']]
print(f'{len(results)-len(failed)}/{len(results)} exact runtime checks')
if failed:
    print(json.dumps(failed,indent=2))
    raise SystemExit(1)
