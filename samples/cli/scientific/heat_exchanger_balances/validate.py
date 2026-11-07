"""Sequential public-entry validation; never accepts packages or rewrites oracles."""
import argparse, hashlib, json, os, subprocess
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('--cli',required=True)
args=parser.parse_args()
p=Path(__file__).resolve().parent
out=p/'build'/'validation'; out.mkdir(parents=True,exist_ok=True)
def run(name,arguments,data=None):
    r=subprocess.run([args.cli,*arguments],input=data,capture_output=True,timeout=240)
    (out/(name+'.stdout')).write_bytes(r.stdout)
    (out/(name+'.stderr')).write_bytes(r.stderr)
    return r
root=str(p/'main.omg')
results=[]
r=run('check',['--check','--target','linux_x86_64',root])
assert r.returncode==0, (r.returncode,r.stderr.decode(errors='replace'))
r=run('intended_invalid',['--check',str(p/'controls/out_of_bounds/main.omg')])
assert r.returncode==1 and b'cannot prove index' in r.stderr, (r.returncode,r.stderr)
results.append(dict(name='intended_invalid',exit=r.returncode,reason='cannot prove index'))
cases=json.loads((p/'data/oracle.json').read_text())['cases']
cases+=json.loads((p/'data/supplemental_oracle.json').read_text())['cases']
for case in cases:
    name=case['name']; data=(p/'data'/(name+'.txt')).read_bytes()
    assert hashlib.sha256(data).hexdigest()==case['input_sha256']
    r=run(name,['run','--both',root],data)
    expected=(p/'data'/(name+'.stdout')).read_bytes()
    native_ok=r.returncode==case['exit'] and r.stdout==expected
    diagnostic=r.stderr.decode(errors='replace')
    # Retain exact diagnostics; a decline is never promoted to agreement.
    terminal='declined' if 'DECLINED' in diagnostic else 'agrees' if f"interp exit: {case['exit']}" in diagnostic and 'DIVERGENCE' not in diagnostic else 'unconfirmed'
    results.append(dict(name=name,exit=r.returncode,native_output_matches=native_ok,terminal=terminal))
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    assert native_ok,(name,r.returncode,r.stdout,diagnostic)
    assert terminal=='agrees',(name,diagnostic)
print(f'{len(results)-1} input cases verified; intended-invalid bounds rejection verified')
