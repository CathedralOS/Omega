"""Independent exact risk-set oracle and subset-enumeration log-rank moments.
Frozen before first sample execution. Time is an exact decimal Fraction.
No compiler or sample source is imported.
"""
from fractions import Fraction as Q
from itertools import combinations
import json, hashlib
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
def analyze(records):
    tables=[]
    for group in (-1,0,1):
        selected=[r for r in records if group == -1 or r[2]==group]
        survival=Q(1); greenwood=Q(0); median=None; rows=[]
        for time in sorted({r[0] for r in selected}):
            risk=sum(r[0]>=time for r in selected)
            event=sum(r[0]==time and r[1]==1 for r in selected)
            censor=sum(r[0]==time and r[1]==0 for r in selected)
            survival*=Q(risk-event,risk)
            if event<risk: greenwood+=Q(event,risk*(risk-event))
            variance=survival**2*greenwood if survival else Q(0)
            if median is None and survival<=Q(1,2): median=time
            rows.append([str(time),risk,event,censor,str(survival),str(variance),int(survival==0)])
        tables.append({'group':group,'rows':rows,'median':str(median) if median is not None else None,'empty':not selected})
    observed=Q(0); expected=Q(0); variance=Q(0)
    for time in sorted({r[0] for r in records if r[1]}):
        risk=[r for r in records if r[0]>=time]
        deaths=sum(r[0]==time and r[1] for r in records)
        observed+=sum(r[0]==time and r[1] and r[2]==0 for r in records)
        # Enumerate every equally likely event subset independently of the
        # closed-form hypergeometric expression used by the Omega sample.
        frequencies={}
        for subset in combinations(range(len(risk)),deaths):
            count=sum(risk[i][2]==0 for i in subset)
            frequencies[count]=frequencies.get(count,0)+1
        total=sum(frequencies.values())
        mean=sum(Q(k*v,total) for k,v in frequencies.items())
        moment=sum(Q(k*k*v,total) for k,v in frequencies.items())
        expected+=mean; variance+=moment-mean*mean
    delta=observed-expected
    return {'tables':tables,'logrank':[str(observed),str(expected),str(variance),str(delta*delta/variance) if variance else None]}
def freeze():
    cases={
      'tied':[(0,1,0),(1,1,0),(1,0,0),(1,1,1),(2,0,1),(3,1,1)],
      'all_censored':[(0,0,0),(1,0,1),(1,0,0),(2,0,1)],
      'complete_failure':[(0,1,0),(0,1,1),(1,1,0),(2,1,1)],
      'simultaneous_failure':[(1,1,0),(1,1,1)],
      'single_censor':[(0,0,0)], 'single_event':[(0,1,1)],
      'empty_group':[(1,1,0),(2,0,0),(3,1,0)],
      'decimal_times':[(Q('0.125'),1,0),(Q('0.125'),0,1),(Q('1.5'),1,1)],
      'maximum':[(i, int(i==31),i%2) for i in range(32)],
    }
    cases['swap']=[(t,e,1-g) for t,e,g in cases['tied']]
    cases['permutation']=list(reversed(cases['tied']))
    for index in range(8):
        cases[f'variant_{index}']=[(Q((i*3+index)%5,2),(i+index)%2,(i//2+index)%2) for i in range(1+index)]
    expected={}
    for name, rows in cases.items():
        rows=[(Q(t),e,g) for t,e,g in rows]
        directory=ROOT/'data'/name; directory.mkdir(parents=True,exist_ok=True)
        def decimal(q): return format(float(q),'.6f').rstrip('0').rstrip('.') if q.denominator!=1 else str(q.numerator)
        directory.joinpath('input.txt').write_text(str(len(rows))+'\n'+''.join(f'{decimal(t)} {e} {g}\n' for t,e,g in rows))
        directory.joinpath('expected_exit.txt').write_text('0\n')
        expected[name]=analyze(rows)
    invalid={'zero':'0\n','over_count':'33\n','negative_count':'-1\n','fraction_count':'1.5\n','negative_time':'1\n-1 1 0\n','nan_time':'1\nnan 1 0\n','infinite_time':'1\ninf 1 0\n','over_time':'1\n1000001 1 0\n','invalid_event':'1\n1 2 0\n','invalid_group':'1\n1 1 2\n','fraction_event':'1\n1 .5 0\n','fraction_group':'1\n1 1 .5\n','missing':'1\n1 1\n','trailing':'1\n1 1 0\n9\n','empty':'','too_precise':'1\n0.0000001 1 0\n','over_budget':' '*4097}
    for name, wire in invalid.items():
        name='invalid_'+name; directory=ROOT/'data'/name; directory.mkdir(parents=True,exist_ok=True)
        directory.joinpath('input.txt').write_text(wire); directory.joinpath('expected_exit.txt').write_text('2\n')
        expected[name]={'error':'ERROR invalid input\n'}
    (ROOT/'validation'/'expected.json').write_text(json.dumps(expected,indent=2)+'\n')
    (ROOT/'validation'/'cases.txt').write_text(''.join(name+'\n' for name in expected))
    paths=[Path(__file__),ROOT/'validation'/'expected.json',ROOT/'validation'/'cases.txt',*sorted((ROOT/'data').glob('*/*'))]
    (ROOT/'validation'/'oracle-freeze.sha256').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(ROOT))+'\n' for p in paths))
if __name__=='__main__': freeze()
