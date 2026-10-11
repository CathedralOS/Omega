#!/usr/bin/env python3
"""Validate an already captured complete application output against frozen data."""
from fractions import Fraction as F
from pathlib import Path
import hashlib,json,sys
ROOT=Path(__file__).resolve().parent

def validate(identifier,horizon,path):
    oracle_path=ROOT/'data/oracle.json'
    freeze=json.loads((ROOT/'data/freeze.json').read_text())
    assert hashlib.sha256(oracle_path.read_bytes()).hexdigest()==freeze['oracle_sha256'],'oracle changed'
    assert hashlib.sha256((ROOT/'reference.py').read_bytes()).hexdigest()==freeze['reference_sha256'],'reference changed'
    oracle=json.loads(oracle_path.read_text())
    lines=Path(path).read_text().splitlines()
    begin=lines.index('transmission_line_reflections v1 micro-units')
    end=lines.index('END',begin)
    assert begin==0 and end==len(lines)-1,'unexpected output outside envelope'
    assert end-begin-1==3*horizon,'record count'
    actual=[line.split(',') for line in lines[begin+1:end]]
    expected=oracle['cases'][str(identifier)]['rows'][:3*horizon]
    for observed,wanted in zip(actual,expected):
        assert observed[0]==wanted[0] and int(observed[1])==wanted[1],(observed,wanted)
        assert len(observed)==len(wanted),(observed,wanted)
        for index,(value,exact) in enumerate(zip(observed[2:],wanted[2:])):
            tolerance=F(oracle['residual_tolerance'] if wanted[0]=='E' and index==4 else oracle['absolute_tolerance'])
            assert abs(F(int(value),oracle['scale'])-F(exact))<=tolerance,(wanted[0],wanted[1],index,value,exact)
    print(f'verified case={identifier} horizon={horizon} records={len(actual)} against frozen exact fractions')
if __name__=='__main__':
    validate(int(sys.argv[1]),int(sys.argv[2]),sys.argv[3])
