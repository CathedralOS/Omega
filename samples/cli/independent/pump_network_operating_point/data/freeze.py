"""Independent exact Fraction oracle, frozen before any Omega execution.
Enumerates every 1/1024 interval, independently of the Omega coarse scan.
"""
import hashlib, itertools, json
from fractions import Fraction
from pathlib import Path
ROOT = Path(__file__).resolve().parent

def oracle(values):
    a,b,c,d,r1,r2,e,m,dem = values
    if not (0<=a<=64 and -16<=b<=16 and 0<=c<=16 and 1<=d<=16 and 1<=r1<=16 and 1<=r2<=16 and 0<=e<=64 and 1<=m<=16 and 0<=dem<=16):
        return {'exit':64,'stdout':'INVALID_INPUT\n'}
    A = -c*(r1+r2); B=b*(r1+r2)-d*r1*r2; C=(a-e*d)*(r1+r2)
    if A==B==C==0:
        return {'exit':64,'stdout':'INVALID_INPUT\n'}
    def difference(tick):
        flow=Fraction(tick,1024)
        return Fraction(a+b*flow-c*flow*flow,d)-e-Fraction(r1*r2,r1+r2)*flow
    intervals=[]
    if A and B*B-4*A*C==0:
        vertex=Fraction(-B,2*A)
        if 0<=vertex<=m:
            tick=vertex*1024
            low=tick.numerator//tick.denominator
            intervals.append((low,low if tick.denominator==1 else low+1))
    else:
        previous=difference(0)
        if not previous: intervals.append((0,0))
        for tick in range(1,m*1024+1):
            current=difference(tick)
            if not current: intervals.append((tick,tick))
            elif previous*current<0: intervals.append((tick-1,tick))
            previous=current
    lines=[]; feasible=0
    for low,high in intervals:
        feasible_here=int(low>=dem*1024); feasible+=feasible_here
        branch=r1+r2
        energy=((a-e*d)*1048576+b*low*1024-c*low*low)*branch-d*r1*r2*low*1024
        assert Fraction(energy,d*1048576*branch)==difference(low)
        assert high-low<=1
        # Frozen tolerance: flow interval width <= 1/1024; reporting fractions
        # exact. Lipschitz bound on |pump-system head residual| <= 800/1024.
        assert abs(difference(low))<=Fraction(800,1024)
        row=[low,high,low*r2,low*r1,1024*branch,e*1024*branch+low*r1*r2,1024*branch,a*1048576+b*low*1024-c*low*low,d*1048576,energy,d*1048576*branch,feasible_here]
        lines.append('ROOT '+' '.join(map(str,row))+'\n')
    lines.append(f'SUMMARY {len(intervals)} {feasible} \n')
    if not feasible: lines.append('INFEASIBLE\n')
    return {'exit':0 if feasible else 2,'stdout':''.join(lines)}

def freeze():
    cases=[]
    named={
        'rational':[20,0,1,1,2,2,0,8,0],
        'asymmetric':[7,0,1,1,2,6,0,8,0],
        'irrational':[24,0,1,1,2,2,0,8,0],
        'multiple':[0,7,1,1,2,2,5,8,0],
        'close_to_shutoff':[0,1,16,1,1,2,0,4,0],
        'two_in_one_bracket':[1,7,8,2,1,2,1,4,0],
        'tangent':[0,5,1,1,2,2,4,8,0],
        'shutoff':[0,0,1,1,2,2,0,8,0],
        'no_intersection':[0,0,1,1,2,2,4,8,0],
        'infeasible_demand':[20,0,1,1,2,2,0,8,5],
        'one_feasible_root':[0,7,1,1,2,2,5,8,3],
        'linear':[12,0,0,2,2,6,0,8,0],
        'upper_endpoint':[20,0,1,1,2,2,0,4,4],
        'rational_denominator':[24,0,1,2,2,2,0,8,0],
        'continuum':[0,1,0,1,2,2,0,8,0],
        'bad_denominator':[20,0,1,0,2,2,0,8,0],
        'closed_branch':[20,0,1,1,0,2,0,8,0],
        'negative_demand':[20,0,1,1,2,2,0,8,-1],
        'excess_capacity':[20,0,1,1,2,2,0,17,0],
    }
    # Enumerated coefficient fixtures are small and exhaustive, not random.
    for a,b,c in itertools.product(range(4),range(4),range(3)):
        named[f'exhaustive_{a}_{b}_{c}']=[a,b,c,1,2,2,1,4,0]
    # A double root at Q=1/3, which is not on the reporting grid.
    named['fractional_tangent']=[0,7,9,1,2,2,1,4,0]
    for name,values in named.items():
        text=' '.join(map(str,values))+'\n'
        cases.append({'name':name,'input':text,'values':values,**oracle(values)})
    for name,text in [('empty',''),('short','1 2\n'),('nondigit','20 x 1 1 2 2 0 8 0\n'),('sign_only','- 0 1 1 2 2 0 8 0\n'),('extra','20 0 1 1 2 2 0 8 0 1\n'),('too_long',' '*96+'\n'),('too_many_digits','10000 0 1 1 2 2 0 8 0\n')]:
        cases.append({'name':name,'input':text,'exit':64,'stdout':'INVALID_INPUT\n'})
    cases.append({'name':'eof_terminator','input':'20 0 1 1 2 2 0 8 0',**oracle([20,0,1,1,2,2,0,8,0])})
    payload={'model':'quadratic pump, parallel linear valves; synthetic only','flow_scale':1024,'flow_tolerance':'1/1024','head_residual_tolerance':'800/1024','reporting':'exact integer numerators and denominators','cases':cases}
    (ROOT/'expected.json').write_text(json.dumps(payload,indent=2)+'\n')
    (ROOT/'freeze.sha256').write_text(hashlib.sha256((ROOT/'expected.json').read_bytes()).hexdigest()+'  expected.json\n')
    print(f'Frozen {len(cases)} cases')

if __name__=='__main__':freeze()
