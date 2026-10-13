"""Independent 80-digit Decimal root oracle; binary roots also checked analytically.
Run --freeze only before compiling or running the application.
"""
from decimal import Decimal as D, localcontext
from pathlib import Path
import json, hashlib, sys
P=Path(__file__).resolve().parent

def solve(z, k):
    with localcontext() as context:
        context.prec=80
        z=list(map(D,z)); k=list(map(D,k)); a=[v-1 for v in k]
        def rr(b): return sum(v*t/(1+b*t) for v,t in zip(z,a))
        if all(t==0 for v,t in zip(z,a) if v):
            return dict(status='non_identifiable',exit=3,feed=list(map(str,z)))
        f0,f1=rr(D(0)),rr(D(1))
        if f0<=0: status='bubble_boundary' if f0==0 else 'liquid'; b=D(0)
        elif f1>=0: status='dew_boundary' if f1==0 else 'vapor'; b=D(1)
        else:
            status='two_phase'; low,high=D(0),D(1)
            # Decimal safeguarded Newton uses the derivative, unlike runtime bisection.
            b=D('.5')
            for _ in range(320):
                f=rr(b)
                if f>0: low=b
                else: high=b
                derivative=-sum(v*t*t/(1+b*t)**2 for v,t in zip(z,a))
                next_b=b-f/derivative
                b=next_b if low<next_b<high else (low+high)/2
                if high-low<D('1e-70'): break
            if len(z)==2:
                analytic=-(z[0]*a[0]+z[1]*a[1])/(a[0]*a[1])
                assert abs(b-analytic)<D('1e-65')
        x=[v/(1+b*t) for v,t in zip(z,a)] if b<1 else None
        y=[q*v for q,v in zip(k,x)] if 0<b<1 else z if b==1 else None
        return dict(status=status,exit=0,beta=str(b),feed=list(map(str,z)),liquid=list(map(str,x)) if x is not None else None,vapor=list(map(str,y)) if y is not None else None)

valid=[('binary_half',['.5','.5'],['2','.5']),('binary_quarter',['.25','.75'],['4','.5']),('binary_nontrivial',['.4','.6'],['3','.3']),('liquid',['.4','.6'],['.2','.8']),('vapor',['.4','.6'],['2','5']),('bubble',['.2','.8'],['3','.5']),('dew',['.8','.2'],['2','.333333333333333333333333333333']),('dew_exact',['.5','.5'],['2','.666666666666666666666666666667']),('dew_binary',['.666666666666666666666666666667','.333333333333333333333333333333'],['2','.5']),('unity',['.3','.7'],['1','1']),('zero_abundance',['0','.4','.6'],['20','3','.3']),('zero_unity',['0','.4','.6'],['20','1','1']),('ternary',['.2','.3','.5'],['.1','1.5','8']),('eight',['.05','.1','.15','.2','.05','.1','.15','.2'],['.05','.2','.7','1','1.8','3','8','20']),('limits',['.5','.5'],['.05','20']),('bubble_exact',['.5','.5'],['1.5','.5']),('dew_exact_finite',['.5','.5'],['.75','1.5'])]
# Restrict inputs to at most 15 significant decimal digits; exact finite boundaries above.
valid=[v for v in valid if v[0] not in ('dew','dew_exact','dew_binary')]
valid += [('reordered',list(reversed(valid[9][1])),list(reversed(valid[9][2])))]
valid += [('bubble_decimal',['.2','.8'],['1.8','.8']),('bubble_decimal_reordered',['.8','.2'],['.8','1.8']),('dew_decimal',['.8','.2'],['1.2','.6']),('near_bubble_inside',['.20000001','.79999999'],['3','.5']),('near_bubble_outside',['.19999999','.80000001'],['3','.5']),('near_dew_inside',['.49999999','.50000001'],['.75','1.5']),('near_dew_outside',['.50000001','.49999999'],['.75','1.5'])]
invalid={'empty':'','count_one':'1 1 1\n','count_nine':'9 '+ '0.125 1 '*9+'\n','negative':'2 -0.5 2 1.5 .5\n','sum':'2 .4 2 .4 .5\n','fraction_high':'2 1.1 2 0 .5\n','k_low':'2 .5 .049 .5 2\n','k_high':'2 .5 20.001 .5 2\n','nan':'2 .5 NaN .5 2\n','infinity':'2 .5 inf .5 2\n','missing':'2 .5 2 .5\n','extra':'2 .5 2 .5 .5 1\n','bad_digit':'2 .5x 2 .5 .5\n','double_dot':'2 .5.0 2 .5 .5\n','oversize':' '*1024,'long_token':'2 0.'+'0'*33+' 2 1 .5\n','count_fraction':'2.5 .5 2 .5 .5\n'}
valid += [('byte_1023',['.5','.5'],['2','.5']),('token_32',['0.'+'0'*30,'1'],['2','.5']),('no_newline',['.5','.5'],['2','.5']),('trailing_dot',['0.','1.'],['2.','.5']),('mixed_whitespace',['.5','.5'],['2','.5'])]
invalid.update(plus='2 +.5 2 .5 .5\n',exponent='2 5e-1 2 .5 .5\n',dot_only='2 . 2 1 .5\n',zero_total='2 0 2 0 .5\n',k_zero='2 .5 0 .5 .5\n')
if __name__=='__main__':
    assert sys.argv[1:]==['--freeze']
    cases=[]
    for name,z,k in valid:
        text=str(len(z))+' '+' '.join(v for pair in zip(z,k) for v in pair)+'\n'
        if name=='byte_1023': text=text+' '*(1023-len(text))
        if name=='no_newline': text=text.rstrip('\n')
        if name=='mixed_whitespace': text=text.replace(' ','\t').replace('\n','\r\n')
        cases.append(dict(name=name,input=text,expected=solve(z,k)))
    for name,text in invalid.items(): cases.append(dict(name=name,input=text,expected=dict(status='invalid',exit=64)))
    raw=(json.dumps(dict(method='80-digit safeguarded Newton plus analytic binary',cases=cases),indent=2)+'\n').encode()
    (P/'data/oracle.json').write_bytes(raw)
    (P/'data/oracle.sha256').write_text(hashlib.sha256(raw).hexdigest()+'  oracle.json\n')
    for case in cases: (P/'data'/(case['name']+'.txt')).write_text(case['input'])
    print(len(cases),'oracles frozen before execution')
