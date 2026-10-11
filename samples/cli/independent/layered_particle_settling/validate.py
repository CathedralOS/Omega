#!/usr/bin/env python3
"""Sequential public-CLI validation against a pre-execution frozen oracle.
Never performs or retries package acceptance. Runtime artifacts stay in build/.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parent

def compare(actual, expected, tolerance):
    left,right=actual.splitlines(),expected.splitlines()
    assert len(left)==len(right), f'row count {len(left)} != {len(right)}'
    for line,witness in zip(left,right):
        words,oracle=line.split(),witness.split()
        assert len(words)==len(oracle), (line,witness)
        for column,(observed,wanted) in enumerate(zip(words,oracle)):
            if wanted.startswith(('+','-')) and wanted[1:].isdigit():
                assert len(observed)==9 and observed[0] in '+-' and observed[1:].isdigit(), observed
                numeric_tolerance = tolerance if (words[0]=="TRACE" and column>=3) or (words[0]=="BALANCE" and column>=2) else 0
                assert abs(int(observed)-int(wanted)) <= numeric_tolerance, (line,witness)
            else:
                assert observed==wanted, (line,witness)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega', required=True)
    parser.add_argument('--route', choices=['check','native','terminal','invalid-source'], required=True)
    arguments=parser.parse_args()
    binary=Path(arguments.omega).resolve()
    output=ROOT/'build'/arguments.route
    output.mkdir(parents=True,exist_ok=True)
    frozen=ROOT/'data/frozen.json'
    expected_hash=(ROOT/'data/frozen.sha256').read_text().split()[0]
    assert hashlib.sha256(frozen.read_bytes()).hexdigest()==expected_hash, 'oracle changed'
    oracle=json.loads(frozen.read_text())
    rows=[]
    def run(name, command, data=None):
        completed=subprocess.run(command,input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=300)
        (output/(name+'.stdout')).write_bytes(completed.stdout)
        (output/(name+'.stderr')).write_bytes(completed.stderr)
        rows.append({'name':name,'command':command,'exit':completed.returncode})
        return completed
    try:
        if arguments.route=='check':
            result=run('check',[str(binary),'--check','--target','linux_x86_64','--build-dir',str(output),str(ROOT/'main.omg')])
            assert result.returncode==0,result.stderr.decode(errors='replace')
        elif arguments.route=='invalid-source':
            control=ROOT/'controls/missing_layer_bound'
            result=run('missing_layer_bound',[str(binary),'--check','--build-dir',str(output),str(control/'main.omg')])
            diagnostic=result.stderr.decode(errors='replace')+result.stdout.decode(errors='replace')
            assert result.returncode==1, (result.returncode,diagnostic)
            assert (control/'expected_error.txt').read_text().strip() in diagnostic, diagnostic
            assert 'unimplemented:' not in diagnostic,diagnostic
        else:
            if arguments.route=='native':
                result=run('compile',[str(binary),'--target','linux_x86_64','--build-dir',str(output),str(ROOT/'main.omg')])
                assert result.returncode==0,result.stderr.decode(errors='replace')
                executables=[p for p in output.rglob('*') if p.is_file() and os.access(p,os.X_OK) and p.suffix not in ('.o','.obj')]
                assert len(executables)==1,executables
                command=[str(executables[0])]
            else:
                command=[str(binary),'run','--both',str(ROOT/'main.omg')]
            cases=list(oracle['cases'].items())
            # LF, CRLF, EOF and signed parser boundaries are all owned here.
            cases.extend([('eof',dict(oracle['cases']['settle'])),('crlf',dict(oracle['cases']['settle']))])
            for name,case in cases:
                data=f"{case['mode']} 0{case['density']}".encode()+ (b'' if name=='eof' else b'\r\n' if name=='crlf' else b'\n')
                result=run(name,command,data)
                assert result.returncode==case['exit'],(name,result.returncode,result.stderr.decode(errors='replace'))
                compare(result.stdout.decode(),case['stdout'],oracle['tolerances']['integer_microunits'])
                if arguments.route=='terminal':
                    diagnostic=result.stderr.decode()
                    assert 'DECLINED' not in diagnostic and 'DIVERGENCE' not in diagnostic and 'FAILED' not in diagnostic,diagnostic
                    assert 'interp exit:' in diagnostic,diagnostic
            invalid=json.loads((ROOT/'data/invalid.json').read_text())
            for name,data in invalid['inputs'].items():
                result=run('invalid_'+name,command,data.encode())
                assert result.returncode==invalid['exit'],(name,result.returncode,result.stderr.decode(errors='replace'))
                assert result.stdout.decode()==invalid['stdout'],(name,result.stdout)
                if arguments.route=='terminal':
                    assert 'DECLINED' not in result.stderr.decode(),result.stderr
        print('PASS',arguments.route,len(rows),'invocations')
    finally:
        (output/'commands.json').write_text(json.dumps({'binary':str(binary),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'oracle_sha256':expected_hash,'invocations':rows},indent=2)+'\n')

if __name__=='__main__':
    main()
