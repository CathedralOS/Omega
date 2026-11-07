"""Qualify frozen expected values and observed native/captured Terminal fields."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import time
sys.dont_write_bytecode = True
from reference import oracle, fixtures
ROOT=Path(__file__).resolve().parent

def check(data,case):
    fields={}
    for line in data.decode().splitlines():
        key,value=line.split('='); fields.setdefault(key,[]).append(float(value))
    assert set(fields)==set(case['expected']),fields
    for key,expected in case['expected'].items():
        expected=expected if isinstance(expected,list) else [expected]
        assert len(fields[key])==len(expected),(key,fields)
        assert all(math.isfinite(a) and math.isclose(a,b,rel_tol=1e-7,abs_tol=1e-7) for a,b in zip(fields[key],expected)),(key,fields[key],expected)
    x=fields['reconstruction']; hx=fields['reconvolution']; n=len(x); k=case['kernel']; lam=case['regularization']; y=case['y']
    def convolve(v,adjoint=False):
        return [sum(w*v[(i+(j-len(k)//2 if adjoint else len(k)//2-j))%n] for j,w in enumerate(k)) for i in range(n)]
    recomputed=convolve(x)
    assert all(math.isclose(a,b,abs_tol=1e-7,rel_tol=1e-7) for a,b in zip(hx,recomputed))
    normal=convolve([a-b for a,b in zip(recomputed,y)],True)
    normal=[normal[i]+lam*(2*x[i]-x[(i-1)%n]-x[(i+1)%n]) for i in range(n)]
    scale=max(1,math.sqrt(sum(v*v for v in convolve(y,True))))
    ratio=math.sqrt(sum(v*v for v in normal))/scale
    assert ratio<=1e-8,(case['name'],'normal equations',ratio)
    assert math.isclose(sum(x),sum(y),abs_tol=1e-7,rel_tol=1e-7),(case['name'],'preserved sum',sum(x),sum(y))
    if case['name']=='signed': assert min(x)<-0.1,'negative reconstruction was clipped'
    return ratio

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--cli',required=True);parser.add_argument('--batch-runner');parser.add_argument('--evidence',type=Path,required=True);args=parser.parse_args()
    args.evidence.mkdir(parents=True,exist_ok=True)
    for name,digest in json.loads((ROOT/'oracle-freeze.json').read_text()).items():
        assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,'oracle freeze changed'
    cases=json.loads((ROOT/'oracles.json').read_text())
    assert cases == fixtures(), 'analytic or independent fixture checks changed'
    for c in cases:
        recalculated=oracle(c['y'],c['kernel'],c['regularization'])
        assert recalculated==c['expected'],'frozen Fourier oracle changed'
    commands=[]
    def run(name,command,data=b''):
        started=time.monotonic()
        result=subprocess.run(list(map(str,command)),input=data,capture_output=True,timeout=180)
        seconds=time.monotonic()-started
        (args.evidence/(name+'.stdout')).write_bytes(result.stdout);(args.evidence/(name+'.stderr')).write_bytes(result.stderr)
        commands.append(dict(name=name,command=list(map(str,command)),exit=result.returncode,seconds=seconds))
        (args.evidence/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
        return result
    cli=Path(args.cli).resolve();source=ROOT/'main.omg'
    refused=run('source_invalid',[cli,'--check',ROOT/'invalid/main.omg'])
    assert refused.returncode==1, refused.stderr
    for text in json.loads((ROOT/'invalid/sample.json').read_text())['rejection_contains']:
        assert text.encode() in refused.stderr, refused.stderr
    valid=cases[-1]['input']
    zero=cases[-1]
    for name,text in [('byte_limit_4095',valid+' '*(4095-len(valid))),('eof_after_digit',valid.rstrip()),('digits_100',valid.rstrip()[:-1]+'0'*100)]:
        cases.append({**zero,'name':name,'input':text})
    invalid=['',valid[:-3],valid+'1',valid.replace('32 3','7 3'),valid.replace('32 3','33 3'),valid.replace('32 3','8.5 3'),valid.replace('32 3','32 2'),valid.replace('32 3','32 7'),valid.replace('100','0',1),valid.replace('100','0.00001',1),valid.replace('100','101',1),valid.replace('0 1 0','0 0.9 0',1),valid.replace('0 1 0','0 -1 0',1),valid.replace('0 1 0','0 1.1 0',1),valid.replace('100','nan',1),valid.replace('100','inf',1),valid.replace('100','1e309',1),valid.replace('0 0 0 0 0', '-1 0 0 0 0',1),valid+'?',valid+' '*4096,valid.replace('100','1..0',1),valid.replace('100','1.',1),valid.replace('100','1'+ '0'*101,1),valid+' '*(4096-len(valid)),valid.rstrip()[:-1]+'0'*101]
    cases += [dict(name=f'invalid_{i}',input=text,exit=2) for i,text in enumerate(invalid)]
    inputs=[]
    input_directory=args.evidence/'inputs';input_directory.mkdir(exist_ok=True)
    for c in cases:
        capture=input_directory/(c['name']+'.stdin');capture.write_text(c['input']);inputs.append(capture)
    if args.batch_runner:
        result=run('batch',[Path(args.batch_runner).resolve(),source,args.evidence,*inputs])
    else:
        result=run('batch',[cli,'run','--both','--keep',source,*[argument for capture in inputs for argument in ('--input',capture)],'--outputs',args.evidence])
    assert result.returncode==0,result.stderr.decode()
    statuses={}
    if not args.batch_runner:
        with (args.evidence/'results.tsv').open(newline='') as table:
            rows=list(csv.DictReader(table,delimiter='\t'))
        assert len(rows)==len(cases),rows
        statuses={row['case']:row for row in rows}
        assert set(statuses)=={c['name'] for c in cases},statuses
    records=[]
    for c in cases:
        data=c['input'].encode();status=c.get('exit',0)
        if args.batch_runner:
            directory=args.evidence/c['name']
            native_stdout=(directory/'native.stdout').read_bytes()
            terminal=(directory/'terminal.stdout').read_bytes()
            assert int((directory/'native.status').read_text())==status
            terminal_status=(directory/'terminal.status').read_text()
            assert terminal_status.startswith('ExternalCompletion(') and f'return_code: {status},' in terminal_status,terminal_status
            assert (directory/'native.stderr').read_bytes()==(directory/'terminal.stderr').read_bytes()==b''
        else:
            row=statuses[c['name']]
            assert row['native']==row['interpreter_exit']==str(status),row
            assert row['interpreter']=='agrees',row
            native_stdout=(args.evidence/(c['name']+'.stdout')).read_bytes()
            terminal=(args.evidence/(c['name']+'.terminal.stdout')).read_bytes()
            assert (args.evidence/(c['name']+'.stderr')).read_bytes()==(args.evidence/(c['name']+'.terminal.stderr')).read_bytes()==b''
        if status:
            assert native_stdout==terminal==b'ERROR malformed input, measurements, kernel or lambda\n',(c['name'],native_stdout,terminal)
        else:
            native_ratio=check(native_stdout,c);terminal_ratio=check(terminal,c)
            records.append(dict(name=c['name'],native_normal_ratio=native_ratio,terminal_normal_ratio=terminal_ratio))
        print(c['name'],status,flush=True)
    (args.evidence/'numerical.json').write_text(json.dumps(records,indent=2)+'\n')
    print(f'{len(records)} numerical cases and {len(invalid)} runtime refusals qualified in both engines')
if __name__=='__main__':main()
