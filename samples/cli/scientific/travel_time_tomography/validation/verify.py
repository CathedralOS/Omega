#!/usr/bin/env python3
"""Independent rational oracle plus complete native/Terminal application runner.

Freeze oracle.json before compiler invocation. This runner never changes it.
Each recorded field is certified by exact convex KKT conditions; rank uses ray
cross products rather than the implementation's Gram determinant test.
"""
import argparse
from fractions import Fraction
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]

def certify(oracle):
    for case in oracle['valid']:
        raw = (ROOT / 'data' / (case['name'] + '.txt')).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == case['input_sha256']
        tokens = list(map(int, raw.split()))
        rows = [tokens[i:i+3] for i in (4,7,10)]
        field = list(map(Fraction, case['field']))
        residuals = [a*field[0]+b*field[1]-t for a,b,t in rows]
        assert residuals == list(map(Fraction,case['residuals']))
        assert sum(r*r for r in residuals) == Fraction(case['sse'])
        rank = 2 if any(a*d-b*c for a,b,_ in rows for c,d,_ in rows) else int(any(a or b for a,b,_ in rows))
        assert rank == case['rank']
        for index,value in enumerate(field):
            low, high = tokens[2*index:2*index+2]
            gradient = sum(row[index]*r for row,r in zip(rows,residuals))
            assert low <= value <= high
            assert low == high or (value == low and gradient >= 0) or (value == high and gradient <= 0) or (low < value < high and gradient == 0)
        if rank < 2 and all(tokens[2*i] < field[i] < tokens[2*i+1] for i in (0,1)):
            # Minimum norm: field perpendicular to every null direction.
            assert all(a*field[1]-b*field[0] == 0 for a,b,_ in rows)
    for case in oracle['invalid']:
        raw = (ROOT / 'data' / (case['name']+'.txt')).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == case['sha256']

def check_output(case,raw):
    output = raw.decode('ascii')
    fields = {k:int(v) for k,v in re.findall(r'^([a-z_0-9]+)=(-?[0-9]+)$',output,re.M)}
    expected = dict(cell_first_micro=case['field'][0],cell_second_micro=case['field'][1],sse_micro=case['sse'])
    expected.update({f'residual_{i}_micro':value for i,value in enumerate(case['residuals'])})
    for key,value in expected.items():
        assert key in fields,(case['name'],'missing',key,output)
        assert abs(Fraction(fields[key],1000000)-Fraction(value)) <= Fraction(2,1000000),(case['name'],key,fields[key],value)
    assert f"rank={case['rank']}; nullity={2-case['rank']}; identifiable={'yes' if case['rank']==2 else 'no'}" in output
    assert 'active_first='+case['active'][0] in output
    assert 'active_second='+case['active'][1] in output
    assert ('fit=consistent_within_1e-12_sse' if Fraction(case['sse']) < Fraction(1,10**12) else 'fit=inconsistent_or_bound_limited') in output
    if case['rank'] < 2:
        direction = [Fraction(fields['null_first_micro'],1000000),Fraction(fields['null_second_micro'],1000000)]
        assert any(direction)
        tokens=list(map(int,(ROOT/'data'/(case['name']+'.txt')).read_text().split()))
        assert all(tokens[i]*direction[0]+tokens[i+1]*direction[1]==0 for i in (4,7,10))
        if case['rank']==0:
            assert 'second null vector=(0,1)' in output

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--compiler',type=Path)
    parser.add_argument('--native',type=Path)
    parser.add_argument('--both',action='store_true')
    parser.add_argument('--captured',type=Path,help='verify complete Terminal stdout/stderr/exit files')
    parser.add_argument('--oracle-only',action='store_true')
    parser.add_argument('--case')
    parser.add_argument('--results',type=Path,default=ROOT/'build'/'validation')
    arguments=parser.parse_args()
    oracle=json.loads((ROOT/'validation'/'oracle.json').read_text())
    certify(oracle)
    print('oracle: 12 exact rational KKT witnesses; 9 hashed invalid fixtures')
    if arguments.oracle_only: return 0
    if arguments.captured:
        for case in oracle['valid']+oracle['invalid']:
            name=case['name']
            raw=(arguments.captured/(name+'.stdout')).read_bytes()
            assert int((arguments.captured/(name+'.exit')).read_text()) == case.get('exit',0),name
            assert (arguments.captured/(name+'.stderr')).read_bytes() == b'',name
            if 'field' in case: check_output(case,raw)
            else: assert raw==b'INVALID: expected 13 integers 0..99, ordered bounds, at most 255 input bytes\n'
            print('Terminal output verified:',name,flush=True)
        return 0
    assert bool(arguments.compiler) != bool(arguments.native),'choose --compiler or --native'
    command=[str(arguments.native.resolve())] if arguments.native else [str(arguments.compiler.resolve()),'run']+(['--both'] if arguments.both else [])+[str(ROOT/'main.omg')]
    arguments.results.mkdir(parents=True,exist_ok=True)
    records=[]
    for case in oracle['valid']+oracle['invalid']:
        if arguments.case and case['name'] != arguments.case: continue
        raw=(ROOT/'data'/(case['name']+'.txt')).read_bytes()
        result=subprocess.run(command,input=raw,capture_output=True,timeout=900 if arguments.compiler else 30)
        (arguments.results/(case['name']+'.stdout')).write_bytes(result.stdout)
        (arguments.results/(case['name']+'.stderr')).write_bytes(result.stderr)
        record=dict(name=case['name'],command=command,exit=result.returncode,stdout_sha256=hashlib.sha256(result.stdout).hexdigest(),stderr_sha256=hashlib.sha256(result.stderr).hexdigest())
        records.append(record)
        (arguments.results/'results.json').write_text(json.dumps(records,indent=2)+'\n')
        assert result.returncode == case.get('exit',0),(record,result.stderr.decode(errors='replace'))
        if arguments.native: assert result.stderr == b'',(case['name'],result.stderr)
        if 'field' in case: check_output(case,result.stdout)
        else: assert result.stdout==b'INVALID: expected 13 integers 0..99, ordered bounds, at most 255 input bytes\n'
        if arguments.both:
            assert (f'interp exit: {result.returncode}'.encode() in result.stderr) or b'DECLINED' in result.stderr, result.stderr
            record['terminal']= 'declined' if b'DECLINED' in result.stderr else 'agreed_termination'
        print('verified:',case['name'], 'exit',result.returncode,flush=True)
    (arguments.results/'results.json').write_text(json.dumps(records,indent=2)+'\n')
    return 0

if __name__=='__main__':
    sys.exit(main())
