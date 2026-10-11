#!/usr/bin/env python3
"""Independent exact rational oracle; never reads an Omega execution to freeze.
The recurrence is evaluated as the affine map q*y+b with interval projection;
Omega evaluates the sampled-feedback Euler expression instead. Equilibria are
all candidates satisfying A*(u-beta*y)==y or the respective rail inequalities.
"""
from fractions import Fraction as F
from pathlib import Path
import json, sys, hashlib
ROOT = Path(__file__).resolve().parent
CASES = {
    'negative_feedback': '4 500 250 -5000 5000 1000 1000 0 12 0',
    'rail_recovery': '4 500 250 -2000 3000 5000 250 4 20 0',
    'lower_rail_recovery': '8 250 125 -1500 3500 -4000 -250 5 24 0',
    'damped_alternation': '4 500 500 -5000 5000 1000 1000 0 16 -3000',
    'positive_feedback': '4 -500 250 -2000 3000 0 0 0 24 100',
    'unstable_timestep': '4 500 1000 -2000 2000 1000 1000 0 10 0',
    'asymmetric_rail': '10 100 200 -1000 2500 2000 2000 0 10 0',
    'rail_boundary': '4 500 250 -2000 3000 2250 2250 0 12 3000',
    'continuum': '4 -250 250 -2000 3000 0 0 0 8 1000',
    'zero_denominator_drift': '4 -250 250 -2000 3000 1000 1000 0 8 0',
    'invalid_rails': '4 500 250 3000 -2000 1000 250 4 12 0',
    'equal_rails': '4 500 250 1000 1000 1000 250 4 12 1000',
    'invalid_gain': '0 500 250 -2000 3000 1000 250 4 12 0',
    'invalid_alpha': '4 500 0 -2000 3000 1000 250 4 12 0',
    'invalid_steps': '4 500 250 -2000 3000 1000 250 4 65 0',
    'invalid_initial': '4 500 250 -2000 3000 1000 250 4 12 4000',
    'invalid_switch': '4 500 250 -2000 3000 1000 250 13 12 0',
    'invalid_feedback': '4 1001 250 -2000 3000 1000 250 4 12 0',
    'invalid_token': '4 500 250 -2000 3000 1e3 250 4 12 0',
    'too_large': '4 500 250 -2000 3000 10001 250 4 12 0',
    'missing_token': '4 500 250 -2000 3000 1000 250 4 12',
    'extra_token': '4 500 250 -2000 3000 1000 250 4 12 0 1',
    'bare_sign': '4 500 250 -2000 3000 - 250 4 12 0',
    'full_line': '1'*193,
}
def expected(text):
    syntax='ERROR syntax: expected ten bounded signed integers\n'
    try:
        tokens=text.split()
        if len(text.encode())+1 >= 192 or len(tokens)!=10 or any(not t.lstrip('-').isdigit() or t.count('-')>1 or ('-' in t and not t.startswith('-')) for t in tokens):
            return 64, syntax
        v=list(map(int,tokens))
        if any(abs(x)>10000 for x in v): return 64,syntax
    except ValueError: return 64,syntax
    gain,beta,alpha,lo,hi,u0,u1,switch,steps,y0=v
    if lo>=hi: return 64,'ERROR rails: lower must be less than upper\n'
    if not (1<=gain<=100 and -1000<=beta<=1000 and 1<=alpha<=1000 and 0<=switch<=steps and 1<=steps<=64 and lo<=y0<=hi):
        return 64,'ERROR parameters: gain, feedback, relaxation, steps or initial output outside bounds\n'
    A=F(gain); beta=F(beta,1000); alpha=F(alpha,1000)
    L,H,U,V,y=map(lambda x:F(x,1000),[lo,hi,u0,u1,y0])
    q=1-alpha*(1+A*beta)
    uv=lambda x:int(x*1000000)
    stable=-1<q<1
    def roots(drive):
        denominator=1+A*beta
        middle=A*drive/denominator if denominator else F(0)
        low=A*(drive-beta*L)<=L
        high=A*(drive-beta*H)>=H
        interior=bool(denominator) and L<middle<H
        continuum=not denominator and not drive
        unique=(int(low)+int(interior)+int(high)==1) and not continuum
        selected=middle if interior else L if low else H
        return low,interior,high,continuum,middle,unique,selected
    rows=['rail_limited_amplifier v1; voltage=uv; q=ppm']
    for phase,drive in enumerate([U,V]):
        low,inside,high,continuum,middle,_,_=roots(drive)
        rows.append(','.join(map(str,['E',phase,int(low),int(inside),int(high),int(continuum),uv(middle),uv(q),int(stable)])))
    *_,unique,target=roots(V)
    rows.append(','.join(map(str,['T',0,uv(U),uv(y),uv(beta*y),uv(y),0])))
    last_bad=-1; overloads=0; recovered=-1
    for n in range(steps+1):
        if n:
            drive=U if n<=switch else V
            feedback=beta*y
            raw=q*y+alpha*A*drive
            y=min(H,max(L,raw))
            overload=-1 if raw<L else 1 if raw>H else 0
            rows.append(','.join(map(str,['T',n,uv(drive),uv(y),uv(feedback),uv(raw),overload])))
            overloads+=bool(overload)
            if n>switch and not overload and recovered==-1: recovered=n
        if n<switch or abs(y-target)>F(1,1000): last_bad=n
    settling=last_bad+1 if stable and unique and last_bad<steps else -1
    rows.append(f'M,{settling},{overloads},{recovered}')
    return 0,'\n'.join(rows)+'\n'

def freeze():
    destination=ROOT/'data'
    for name,text in CASES.items():
        (destination/f'{name}.in').write_text(text+'\n')
        status,stdout=expected(text)
        (destination/f'{name}.expected').write_text(stdout)
        (destination/f'{name}.exit').write_text(str(status)+'\n')
    frozen={
        'model':'y[n+1]=project_[L,H](q*y[n]+alpha*A*u[n+1]); q=1-alpha*(1+A*beta)',
        'scientific_settling_band_volts':'1/1000 inclusive; final-phase suffix observed through requested horizon',
        'presentation_tolerance_microvolts':2,
        'flags_counts_indices_exit_tolerance':0,
        'reference_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'fixtures':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted(destination.iterdir()) if f.suffix in ['.in','.expected','.exit']},
    }
    (destination/'oracle-freeze.json').write_text(json.dumps(frozen,indent=2)+'\n')

def verify(name,path,status):
    frozen=json.loads((ROOT/'data/oracle-freeze.json').read_text())
    assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest()==frozen['reference_sha256'], 'reference modified after freeze'
    for filename,digest in frozen['fixtures'].items():
        assert hashlib.sha256((ROOT/'data'/filename).read_bytes()).hexdigest()==digest, f'oracle modified: {filename}'
    assert int(status)==int((ROOT/'data'/f'{name}.exit').read_text()),'wrong exit'
    want=(ROOT/'data'/f'{name}.expected').read_text().splitlines()
    got=Path(path).read_text().splitlines()
    assert len(got)==len(want),f'wrong row count {len(got)} != {len(want)}'
    for number,(a,b) in enumerate(zip(want,got)):
        x,y=a.split(','),b.split(',')
        assert len(x)==len(y),f'wrong columns at {number}'
        tolerant={2,3,4,5} if x[0]=='T' else {6,7} if x[0]=='E' else set()
        for column,(first,second) in enumerate(zip(x,y)):
            if column in tolerant:
                assert abs(int(first)-int(second))<=frozen['presentation_tolerance_microvolts'],f'row {number} column {column}: {second} != {first}'
            else: assert first==second,f'row {number} column {column}: {second} != {first}'
    print(f'{name}: exit and all {len(want)} rows match frozen oracle')
if __name__=='__main__':
    if sys.argv[1:]==['freeze']: freeze()
    elif len(sys.argv)==5 and sys.argv[1]=='verify': verify(*sys.argv[2:])
    else: raise SystemExit('reference.py freeze | verify CASE OUTPUT EXIT')
