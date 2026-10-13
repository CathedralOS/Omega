"""Independent enthalpy ODE oracle, frozen before any sample execution.
SciPy DOP853 integrates both specific enthalpies independently; it does not
share the production cumulative-transfer or affine-event algorithm.
"""
import hashlib,json,math
from pathlib import Path
import numpy as np
import scipy
from scipy.integrate import solve_ivp
ROOT=Path(__file__).resolve().parents[1]
def temperature(h,c,L,T):return T+h/c if h<0 else T if h<=L else T+(h-L)/c
def fraction(h,L):return min(1.,max(0.,h/L))
def solve(text,tolerance):
    v=list(map(float,text.split()));m1,c1,L1,T1,h1,m2,c2,L2,T2,h2,G,n=v[:12];times=v[12:]
    def rhs(t,h):
        power=G*(temperature(h[0],c1,L1,T1)-temperature(h[1],c2,L2,T2))
        return [-power/m1,power/m2]
    if max(times)==0 or G==0:values=np.array([[h1,h2]]*len(times)).T
    else:
        result=solve_ivp(rhs,[0,max(times)],[h1,h2],method='DOP853',rtol=tolerance,atol=tolerance/10,dense_output=True,max_step=max(times)/200)
        assert result.success
        values=result.sol(times)
    rows=[]
    for t,a,b in zip(times,*values):
        rows.append(dict(time=t,temperature1=temperature(a,c1,L1,T1),temperature2=temperature(b,c2,L2,T2),fraction1=fraction(a,L1),fraction2=fraction(b,L2),enthalpy1=float(a),enthalpy2=float(b),transferred_energy=m1*(h1-a)))
    return rows
CASES={
'solid_analytic':'2 3 40 100 -90 4 5 70 100 -200 6 6 0 0.1 1 5 20 1000',
'liquid_analytic':'3 4 20 10 100 2 3 30 10 60 5 5 0 0.2 2 10 1000',
'melting':'1 2 80 20 -20 2 3 100 20 250 2 8 0 0.1 1 3 5 10 30 1000',
'freezing':'2 3 100 20 150 1 2 80 20 -120 2 8 0 0.1 1 3 5 10 30 1000',
'different_melting':'1 2 60 10 -20 2 3 70 40 160 4 8 0 0.1 1 2 5 10 30 1000',
'both_isothermal':'1 2 100 10 40 2 3 200 20 80 1 6 0 1 3 5 10 1000',
'zero_conductance':'2 3 100 10 -15 3 4 200 20 250 0 4 0 1 100 1000',
'equilibrium_plateau':'2 3 100 20 60 4 5 80 20 30 1000 3 0 1 1000',
'endpoint_heat':'1 1 10 10 0 1 1 10 10 30 1 5 0 0.01 1 10 1000',
'endpoint_cool':'1 1 10 10 10 1 1 10 10 -20 1 5 0 0.01 1 10 1000',
'extreme_rate':'1 1 1 1 -1000 1 1 1 1000 1000 1000 4 0 0.000001 0.01 1000',
'mass_scaled':'2 2 80 20 -20 4 3 100 20 250 4 8 0 0.1 1 3 5 10 30 1000',
}
def freeze():
    cases={}
    for name,text in CASES.items():
        text+='\n';(ROOT/'data'/f'{name}.txt').write_text(text)
        rows=solve(text,2e-13);coarse=solve(text,2e-12)
        for a,b in zip(rows,coarse):
            for key in a:assert abs(a[key]-b[key])<2e-8*max(1,abs(a[key])),(name,key,a[key],b[key])
        v=list(map(float,text.split()));m1,c1,L1,T1,h1,m2,c2,L2,T2,h2,G,n=v[:12]
        energy=m1*h1+m2*h2;scale=max(1,abs(m1*h1)+abs(m2*h2),m1*L1+m2*L2)
        for row in rows:assert abs(m1*row['enthalpy1']+m2*row['enthalpy2']-energy)<1e-10*scale
        analytic=[]
        if name in ('solid_analytic','liquid_analytic'):
            a=temperature(h1,c1,L1,T1);b=temperature(h2,c2,L2,T2);C1=m1*c1;C2=m2*c2;eq=(C1*a+C2*b)/(C1+C2)
            for row in rows:
                difference=(a-b)*math.exp(-G*(1/C1+1/C2)*row['time'])
                wanted1=eq+C2/(C1+C2)*difference;wanted2=eq-C1/(C1+C2)*difference
                assert abs(row['temperature1']-wanted1)<1e-9 and abs(row['temperature2']-wanted2)<1e-9
                analytic.append(dict(temperature1=wanted1,temperature2=wanted2))
        # Equilibrium partition: independent temperature-law root over q;
        # plateau roots need only energy closure and equal temperature.
        last=rows[-1]
        if G>0:
            assert abs(last['temperature1']-last['temperature2'])<1e-8
            if last['temperature1']!=T1 and last['temperature2']!=T2:
                eq=last['temperature1'];offset1=0 if eq<T1 else L1;offset2=0 if eq<T2 else L2
                equilibrium=(energy-m1*offset1-m2*offset2+m1*c1*T1+m2*c2*T2)/(m1*c1+m2*c2)
                assert abs(eq-equilibrium)<1e-8
            else:equilibrium=last['temperature1']
        else:equilibrium=None
        cases[name]=dict(input_sha256=hashlib.sha256(text.encode()).hexdigest(),rows=rows,analytic=analytic,equilibrium_temperature=equilibrium,energy_scale=scale)
    (ROOT/'validation/oracle.json').write_text(json.dumps(dict(scipy=scipy.__version__,method='DOP853 independent two-enthalpy ODE; refined tolerance comparison',cases=cases),indent=2)+'\n')
    manifest={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [Path(__file__),ROOT/'validation/oracle.json',*sorted((ROOT/'data').glob('*.txt'))]}
    (ROOT/'validation/FREEZE.json').write_text(json.dumps(manifest,indent=2)+'\n')
if __name__=='__main__':freeze()
