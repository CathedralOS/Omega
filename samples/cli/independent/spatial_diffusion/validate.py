"""Validate all numerical cells, convergence reports, failures and refinement.

Native execution uses the public CLI. Terminal captures come from the normal
native report's exact retained artifact, executed by the shared sample runner.
The --captured route checks both execution engines against one frozen oracle.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
from reference import problem
ROOT=Path(__file__).resolve().parent

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def oracle():
    path=ROOT/'data/oracle.json'
    assert digest(path)==(ROOT/'data/freeze.sha256').read_text().split()[0], 'frozen oracle modified'
    return json.loads(path.read_text())
def verify(case,stdout,exit_code):
    assert exit_code==case['exit'], (case['name'],exit_code,case['exit'])
    if 'stdout' in case:
        assert stdout==case['stdout'].encode(), (case['name'],stdout)
        return
    lines=stdout.decode('ascii').splitlines()
    assert len(lines)==case['n']**2+3
    header=lines[0].split()
    assert header==['GRID',str(case['n']),str(case['n']**2),str(case['nonzeros'])],header
    report=lines[1].split()
    assert report[0]=='CG' and int(report[1])==(0 if case['exit']==0 else 2)
    iterations=int(report[2]); residual,threshold,error=[int(x)/1e12 for x in report[3:]]
    assert 0<=iterations<=case['limit']
    if case['limit']<=1:assert iterations==case['limit']
    assert abs(threshold-case['threshold'])<=1.1e-12
    assert abs(error-case['error'])<=4e-8
    values=[]
    for index,line in enumerate(lines[2:-1]):
        kind,identifier,value=line.split()
        assert kind=='CELL' and int(identifier)==index
        values.append(int(value)/1e12)
    assert max(abs(a-b) for a,b in zip(values,case['solution']))<=4e-8
    n=case['n']; mixed=case['name']=='mixed'; reaction=case['name']=='reaction'
    matrix,right,exact=problem(n,2. if reaction else 1.,3. if reaction else 0.,mixed)
    measured=max(abs(b-sum(a*x for a,x in zip(row,values))) for row,b in zip(matrix,right))
    # stdout rounds each solution toward zero to 1e-12. Bound propagated error
    # using the infinity matrix norm, rather than relaxing the solver tolerance.
    rounding=max(sum(abs(x) for x in row) for row in matrix)*1e-12
    assert abs(measured-residual)<=rounding+3e-12,(measured,residual,rounding)
    if case['exit']==0:
        assert residual<=threshold+1e-12
        assert measured<=threshold+rounding+3e-12
        assert lines[-1]=='CONVERGED'
    else:
        assert residual>threshold and lines[-1]=='NOT_CONVERGED'
        assert abs(residual-case['residual'])<=4e-8
    return max(abs(value-target) for value,target in zip(values,exact))

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',type=Path)
    parser.add_argument('--native-binary',type=Path)
    parser.add_argument('--captured',type=Path)
    options=parser.parse_args()
    assert sum(bool(value) for value in (options.omega,options.native_binary,options.captured))==1,'choose --omega, --native-binary or --captured'
    frozen=oracle(); out=ROOT/'build/validation/native';out.mkdir(parents=True,exist_ok=True)
    records=[]
    measured_errors={}
    for case in frozen['cases']:
        name=case['name']; started=time.monotonic()
        if options.omega or options.native_binary:
            command=[str(options.omega.resolve()),'run',str(ROOT/'main.omg')] if options.omega else [str(options.native_binary.resolve())]
            process=subprocess.run(command,input=case['input'].encode(),stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=ROOT,timeout=600)
            raw=process.stdout; exit_code=process.returncode
            (out/f'{name}.stdout').write_bytes(raw);(out/f'{name}.stderr').write_bytes(process.stderr)
            (out/f'{name}.exit').write_text(str(exit_code))
            if options.native_binary:assert process.stderr==b'',process.stderr
            assert b'compile FAILED' not in process.stderr and b'DECLINED' not in process.stderr,process.stderr.decode(errors='replace')
        else:
            command=['captured',str(options.captured)]
            raw=(options.captured/f'{name}.stdout').read_bytes()
            exit_code=int((options.captured/f'{name}.exit').read_text())
            assert (options.captured/f'{name}.stderr').read_bytes()==b''
        measured_errors[name]=verify(case,raw,exit_code)
        records.append(dict(name=name,command=command,exit=exit_code,stdout_sha256=hashlib.sha256(raw).hexdigest(),seconds=time.monotonic()-started))
        print(name,'verified',flush=True)
    errors=[measured_errors[name] for name in ['coarse','medium','fine']]
    assert abs(errors[0]/errors[1]-4.)<1e-4 and abs(errors[1]/errors[2]-2.25)<1e-4
    result=dict(measured_refinement_errors=errors,measured_refinement_ratios=[errors[0]/errors[1],errors[1]/errors[2]],oracle_sha256=digest(ROOT/'data/oracle.json'),compiler_sha256=digest(options.omega.resolve()) if options.omega else None,native_sha256=digest(options.native_binary.resolve()) if options.native_binary else None,revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),records=records)
    destination=out/'results.json' if options.omega or options.native_binary else options.captured/'verified.json'
    destination.write_text(json.dumps(result,indent=2)+'\n')
if __name__=='__main__':main()
