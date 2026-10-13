"""Qualify frozen expected values and observed native/captured Terminal fields."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import sys
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
        result=subprocess.run(list(map(str,command)),input=data,capture_output=True,timeout=180)
        (args.evidence/(name+'.stdout')).write_bytes(result.stdout);(args.evidence/(name+'.stderr')).write_bytes(result.stderr)
        commands.append(dict(name=name,command=list(map(str,command)),exit=result.returncode))
        (args.evidence/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
        return result
    cli=Path(args.cli).resolve();source=ROOT/'main.omg'
    refused=run('source_invalid',[cli,'--check',ROOT/'invalid/main.omg'])
    assert refused.returncode==1, refused.stderr
    for text in json.loads((ROOT/'invalid/sample.json').read_text())['rejection_contains']:
        assert text.encode() in refused.stderr, refused.stderr
    if not args.batch_runner:
        compiled=run('compile',[cli,'--target','linux_x86_64',source]);assert compiled.returncode==0,compiled.stderr.decode()
        paths=re.findall(rb'^published native output to (.+)$',compiled.stdout,re.M);assert len(paths)==1,compiled.stdout
        native=Path(paths[0].decode()).resolve()
    valid=cases[-1]['input']
    zero=cases[-1]
    for name,text in [('byte_limit_4095',valid+' '*(4095-len(valid))),('eof_after_digit',valid.rstrip()),('digits_100',valid.rstrip()[:-1]+'0'*100)]:
        cases.append({**zero,'name':name,'input':text})
    invalid=['',valid[:-3],valid+'1',valid.replace('32 3','7 3'),valid.replace('32 3','33 3'),valid.replace('32 3','8.5 3'),valid.replace('32 3','32 2'),valid.replace('32 3','32 7'),valid.replace('100','0',1),valid.replace('100','0.00001',1),valid.replace('100','101',1),valid.replace('0 1 0','0 0.9 0',1),valid.replace('0 1 0','0 -1 0',1),valid.replace('0 1 0','0 1.1 0',1),valid.replace('100','nan',1),valid.replace('100','inf',1),valid.replace('100','1e309',1),valid.replace('0 0 0 0 0', '-1 0 0 0 0',1),valid+'?',valid+' '*4096,valid.replace('100','1..0',1),valid.replace('100','1.',1),valid.replace('100','1'+ '0'*101,1),valid+' '*(4096-len(valid)),valid.rstrip()[:-1]+'0'*101]
    cases += [dict(name=f'invalid_{i}',input=text,exit=2) for i,text in enumerate(invalid)]
    if args.batch_runner:
        inputs=[]
        for c in cases:
            directory=args.evidence/c['name'];directory.mkdir(exist_ok=True)
            capture=directory/'input.txt';capture.write_text(c['input']);inputs.append(capture)
        result=run('batch',[Path(args.batch_runner).resolve(),source,args.evidence,*inputs])
        assert result.returncode==0,result.stderr.decode()
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
            a=run('native_'+c['name'],[native],data)
            b=run('both_'+c['name'],[cli,'run','--both','--keep',source],data)
            retained=re.findall(rb'^build dir kept: (.+)$',b.stderr,re.M);assert len(retained)==1,(c['name'],b.stderr)
            terminal=(Path(retained[0].decode())/'terminal.stdout').read_bytes()
            (args.evidence/('terminal_'+c['name']+'.stdout')).write_bytes(terminal)
            assert a.returncode==b.returncode==status,(c['name'],a.returncode,b.returncode,b.stderr)
            assert f'interp exit: {status}'.encode() in b.stderr and b'DECLINED' not in b.stderr and b'DIVERGENCE' not in b.stderr,b.stderr
            assert a.stdout==b.stdout
            native_stdout=a.stdout
        if status:
            assert native_stdout==terminal==b'ERROR malformed input, measurements, kernel or lambda\n',(c['name'],native_stdout,terminal)
        else:
            native_ratio=check(native_stdout,c);terminal_ratio=check(terminal,c)
            records.append(dict(name=c['name'],native_normal_ratio=native_ratio,terminal_normal_ratio=terminal_ratio))
        print(c['name'],status,flush=True)
    (args.evidence/'numerical.json').write_text(json.dumps(records,indent=2)+'\n')
    print(f'{len(records)} numerical cases and {len(invalid)} runtime refusals qualified in both engines')
if __name__=='__main__':main()
