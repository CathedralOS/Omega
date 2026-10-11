#!/usr/bin/env python3
"""Independent Decimal expanded polynomial and analytic quadratic root oracle.
Freeze generates expectations before any Omega check/execution. Verify is read-only.
"""
from decimal import Decimal, getcontext
from pathlib import Path
import hashlib, json, sys
getcontext().prec = 70
D = Decimal
ROOT = Path(__file__).resolve().parent

def polynomial(s, t):
    return s[2] + s[3] * t + s[4] * t ** 2

def oracle(raw):
    if any(character not in '0123456789- \t\r\n' for character in raw):
        return {'exit': 10, 'status': 'ERROR input'}
    parts = raw.split()
    if len(raw) >= 512 or len(parts) != 15:
        return {'exit': 10, 'status': 'ERROR input'}
    try:
        if any(not (bool(p.lstrip('-')) and all('0' <= digit <= '9' for digit in p.lstrip('-')) and len(p.lstrip('-')) <= 7 and abs(int(p)) <= 1000000) for p in parts):
            raise ValueError()
        v = [D(p) / 1000 for p in parts]
    except (ValueError, ArithmeticError):
        return {'exit': 10, 'status': 'ERROR input'}
    first, second = v[:5], v[5:10]
    def valid(s):
        dl, dh = s[3] + 2*s[4]*s[0], s[3] + 2*s[4]*s[1]
        return -100 <= s[0] < s[1] <= 200 and abs(s[2]) <= 100 and abs(s[3]) <= 1 and abs(s[4]) <= D('.01') and (dl > 0 and dh > 0 or dl < 0 and dh < 0)
    if not all(map(valid, (first,second))) or first[1] != second[0] or abs(polynomial(first,first[1])-polynomial(second,second[0])) >= D('1e-8'):
        return {'exit':11,'status':'ERROR calibration'}
    measured, cold, reference, low, high = v[10:]
    cold_segment = next((s for s in (first, second) if s[0] <= cold <= s[1]), None)
    if cold_segment is None:
        return {'exit':12,'status':'ERROR cold junction'}
    target = measured + polynomial(cold_segment,cold)
    if not first[0] <= low < high <= second[1]:
        return {'exit':13,'status':'ERROR bracket'}
    roots = []
    # Enumerate endpoint enclosures independently, then solve analytically.
    for s in (first,second):
        left,right = max(low,s[0]),min(high,s[1])
        if left > right or not min(polynomial(s,left),polynomial(s,right)) <= target <= max(polynomial(s,left),polynomial(s,right)):
            continue
        a,b,c = s[4],s[3],s[2]-target
        if a == 0:
            candidates = [-c/b]
        else:
            disc = b*b-4*a*c
            candidates = [(-b+disc.sqrt())/(2*a),(-b-disc.sqrt())/(2*a)] if disc >= 0 else []
        for t in candidates:
            if left <= t <= right and not any(abs(t-r) < D('1e-8') for r in roots):
                roots.append(t)
    if len(roots) != 1:
        return {'exit':14 if not roots else 15,'status':'ERROR no root' if not roots else 'ERROR ambiguous','brackets':len(roots)}
    t = roots[0]
    return {'exit':0,'status':'OK','brackets':1,'temperature_mC':str(t*1000),'error_mC':str((t-reference)*1000),'compensated_nV':str(target*1000000),'residual_nV':'0'}

def freeze():
    baseline = [-10000,50000,0,40,1,50000,100000,-5000,240,-1]
    entries={}
    def add(name, obs, calibration=None):
        text = ' '.join(map(str,(calibration or baseline)+obs))+'\n'
        (ROOT/'data'/f'{name}.txt').write_text(text)
        entries[name] = oracle(text)
    add('curved_hot',[5750,25000,74000,-10000,100000])
    add('compensation_zero',[5750,0,60000,-10000,100000])
    add('cold_boundary',[2875,50000,75000,-10000,100000])
    add('hot_boundary',[4500,0,50000,-10000,100000])
    add('lower_endpoint',[-300,0,-10000,-10000,100000])
    add('upper_endpoint',[9000,0,100000,-10000,100000])
    add('negative_temperature',[-175,0,-6000,-10000,100000])
    add('clipped_bracket',[5750,25000,75000,70000,80000])
    add('no_root',[5750,25000,75000,0,40000])
    add('reversed_bracket',[5750,25000,75000,80000,70000])
    add('outside_bracket',[5750,25000,75000,-20000,100000])
    add('cold_outside',[5750,101000,75000,-10000,100000])
    add('discontinuous',[5750,25000,75000,-10000,100000],baseline[:7]+[-4000,240,-1])
    add('turning_segment',[5750,25000,75000,-10000,100000],baseline[:8]+[100,-1])
    # Continuous piecewise linear tent: two distinct roots for the same signal.
    tent=[-10000,50000,0,40,0,50000,100000,4000,-40,0]
    add('ambiguous',[1000,0,25000,-10000,100000],tent)
    add('decreasing_unique',[1000,0,75000,60000,100000],tent)
    add('tent_boundary',[2000,0,50000,-10000,100000],tent)
    invalid={'malformed':'x\n','truncated':'0 1\n','oversize_token':'12345678\n','extra':(ROOT/'data/curved_hot.txt').read_text()+'9\n','budget':' '*512+(ROOT/'data/curved_hot.txt').read_text(),'missing_digit':'- \n'}
    for name,raw in invalid.items():
        (ROOT/'data'/f'{name}.txt').write_text(raw)
        entries[name]=oracle(raw)
    doc={'method':'Decimal precision 70; expanded quadratic; analytical roots and independent per-segment endpoint enclosure; frozen before Omega execution',
         'tolerances':{'temperature_mC':'2','error_mC':'2','compensated_nV':'2','residual_nV':'1'},'cases':entries,
         'fixture_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((ROOT/'data').glob('*.txt'))}}
    (ROOT/'expected.json').write_text(json.dumps(doc,indent=2)+'\n')
    print(hashlib.sha256((ROOT/'expected.json').read_bytes()).hexdigest())

def verify(directory, names=None):
    expected=json.loads((ROOT/'expected.json').read_text())
    for filename,digest in expected['fixture_sha256'].items():
        assert hashlib.sha256((ROOT/'data'/filename).read_bytes()).hexdigest()==digest, filename
    for name,case in expected['cases'].items():
        if names is not None and name not in names: continue
        folder=Path(directory)
        actual=(folder/f'{name}.stdout').read_text().splitlines()
        code=int((folder/f'{name}.exit').read_text())
        assert code==case['exit'], (name,code,case)
        status=[s for s in actual if '=' not in s]
        assert status==[case['status']], (name,status)
        fields=dict(line.split('=',1) for line in actual if '=' in line)
        assert set(fields)==set(case)-{'exit','status'}, (name,fields)
        for key,value in fields.items():
            tolerance=D(expected['tolerances'].get(key,'0'))
            assert abs(D(value)-D(str(case[key])))<=tolerance,(name,key,value,case[key])
    print(f"PASS {len(names) if names is not None else len(expected['cases'])} cases; frozen numeric tolerances and exact statuses/exits")

if __name__=='__main__':
    if sys.argv[1:] == ['freeze']: freeze()
    elif len(sys.argv) in (3,4) and sys.argv[1]=='verify': verify(sys.argv[2], set(sys.argv[3].split(',')) if len(sys.argv)==4 else None)
    else: raise SystemExit('reference.py freeze | verify OUTPUT_DIRECTORY')
