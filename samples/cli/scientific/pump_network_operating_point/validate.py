"""Run the frozen input/output matrix against a published native executable.
No oracle regeneration; each subprocess is serial and has a time limit.
"""
import argparse,hashlib,json,subprocess
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('executable')
parser.add_argument('--record',default='build/native-validation.json')
arguments=parser.parse_args()
root=Path(__file__).resolve().parent
expected=root/'data/expected.json'
digest=hashlib.sha256(expected.read_bytes()).hexdigest()
assert digest== (root/'data/freeze.sha256').read_text().split()[0], 'oracle changed after freeze'
results=[]
for case in json.loads(expected.read_text())['cases']:
    run=subprocess.run([str(Path(arguments.executable).resolve())],input=case['input'],text=True,capture_output=True,timeout=20)
    passed=run.returncode==case['exit'] and run.stdout==case['stdout'] and not run.stderr
    results.append({'name':case['name'],'passed':passed,'expected_exit':case['exit'],'exit':run.returncode,'stdout':run.stdout,'stderr':run.stderr})
record=root/arguments.record
record.parent.mkdir(parents=True,exist_ok=True)
record.write_text(json.dumps({'oracle_sha256':digest,'executable_sha256':hashlib.sha256(Path(arguments.executable).read_bytes()).hexdigest(),'results':results},indent=2)+'\n')
failed=[r['name'] for r in results if not r['passed']]
print(f'{len(results)-len(failed)}/{len(results)} cases matched frozen oracles; failures={failed}')
raise SystemExit(bool(failed))
