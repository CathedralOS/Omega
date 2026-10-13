"""Check actual numbers, refusals and geometry against immutable independent data."""
import argparse,hashlib,json,subprocess
from pathlib import Path
import numpy as np
ROOT=Path(__file__).resolve().parents[1]
ORACLE_SHA='26dc74e85cb7bdad5b02761facda1fc1705679b2183cd3e71bcc63dfdf61fbf3'
def load():
    payload=(ROOT/'data/oracle.json').read_bytes();assert hashlib.sha256(payload).hexdigest()==ORACLE_SHA
    oracle=json.loads(payload)
    boundary=(ROOT/'data/oracle_boundary.json').read_bytes()
    assert hashlib.sha256(boundary).hexdigest()=='2f933f293f54708d7eb55f553643d0478ad953d09e30bde03a5d430ddf16bef1'
    oracle['cases'].update(json.loads(boundary)['cases'])
    for name,case in oracle['cases'].items():assert hashlib.sha256((ROOT/'data'/f'{name}.txt').read_bytes()).hexdigest()==case['input_sha256'],name
    return oracle
def check(stdout,stderr,code,name,oracle):
    case=oracle['cases'][name];assert code==case['exit'],(name,code,stdout,stderr);assert not stderr,(name,stderr)
    if code:
        expected=b'error invalid input\n' if code==64 else b'status non-identifiable\n'
        assert stdout==expected,(name,stdout);return dict(case=name,exit=code)
    lines=stdout.decode().splitlines();expected=case['expected'];assert lines[0]=='status fitted; numbers in 1e-12 units'
    def numbers(line,label,count):
        tokens=line.split();assert tokens[0]==label and len(tokens)==count+1,(name,line)
        return np.array([int(x)/1e12 for x in tokens[1:]])
    rotation=numbers(lines[1],'rotation',9).reshape(3,3);translation=numbers(lines[2],'translation',3)
    points=np.array([numbers(line,'point',7) for line in lines[3:-1]])
    assert len(points)==len(expected['transformed']),(name,points)
    rms=numbers(lines[-1],'rms',1)[0]
    error=max(np.max(np.abs(rotation-expected['rotation'])),np.max(np.abs(translation-expected['translation'])),np.max(np.abs(points[:,:3]-expected['transformed'])),np.max(np.abs(points[:,3:6]-expected['residual'])),np.max(np.abs(points[:,6]-expected['norm'])),abs(rms-expected['rms']))
    orthogonal=float(np.max(np.abs(rotation.T@rotation-np.eye(3))));determinant=abs(float(np.linalg.det(rotation))-1)
    assert error<=1e-7,(name,'numeric',error);assert orthogonal<=1e-8 and determinant<=1e-8,(name,orthogonal,determinant)
    # Recompute the emitted geometry to catch internally inconsistent fields.
    raw=np.array([float(x) for x in (ROOT/'data'/f'{name}.txt').read_text().split()[1:]]).reshape(-1,7)
    transformed=raw[:,:3]@rotation.T+translation;residual=(transformed-raw[:,3:6])*np.sqrt(raw[:,6,None])
    assert np.max(np.abs(transformed-points[:,:3]))<=1e-7 and np.max(np.abs(residual-points[:,3:6]))<=1e-7
    return dict(case=name,exit=code,max_numeric_error=float(error),orthogonality_error=orthogonal,determinant_error=determinant)
def main():
    parser=argparse.ArgumentParser();parser.add_argument('binary',type=Path);parser.add_argument('--output',type=Path,required=True);options=parser.parse_args()
    oracle=load();options.output.mkdir(parents=True,exist_ok=True);results=[]
    for name in oracle['cases']:
        observed=subprocess.run([str(options.binary.resolve())],input=(ROOT/'data'/f'{name}.txt').read_bytes(),capture_output=True,timeout=20)
        for suffix,payload in [('stdout',observed.stdout),('stderr',observed.stderr),('exit',str(observed.returncode).encode())]:(options.output/f'{name}.native.{suffix}').write_bytes(payload)
        results.append(check(observed.stdout,observed.stderr,observed.returncode,name,oracle))
    (options.output/'results.json').write_text(json.dumps(results,indent=2)+'\n');print('native:',len(results),'frozen numerical/refusal cases matched')
if __name__=='__main__':main()
