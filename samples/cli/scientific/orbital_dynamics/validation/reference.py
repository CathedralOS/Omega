"""Independent references: analytic Kepler and SciPy DOP853, never the sample integrator.
Run only to author/freeze references; validation consumes frozen bytes.
"""
import hashlib, json, math
from pathlib import Path
import scipy
from scipy.integrate import solve_ivp
ROOT = Path(__file__).resolve().parents[1]

def kepler(t, eccentricity, reversed=False, angle=0):
    # Start at apoapsis, a=mu=1. Newton solves E-e sin E=M.
    mean = math.pi + (-t if reversed else t)
    anomaly = mean
    for _ in range(20):
        anomaly -= (anomaly-eccentricity*math.sin(anomaly)-mean)/(1-eccentricity*math.cos(anomaly))
    rate = (-1 if reversed else 1)/(1-eccentricity*math.cos(anomaly))
    # Rotate apoapsis onto positive x.
    x, y = eccentricity-math.cos(anomaly), -math.sqrt(1-eccentricity**2)*math.sin(anomaly)
    vx, vy = math.sin(anomaly)*rate, -math.sqrt(1-eccentricity**2)*math.cos(anomaly)*rate
    c,s=math.cos(angle),math.sin(angle)
    return [c*x-s*y,s*x+c*y,c*vx-s*vy,s*vx+c*vy]

def freeze():
    cases={}
    for name,e,rev,angle,steps in [('circular_coarse',0,False,0,640),('circular',0,False,0,1280),('circular_half',0,False,0,2560),('eccentric',.5,False,0,1280),('eccentric_half',.5,False,0,2560),('reversed',.5,True,0,1280),('rotated',.5,False,math.pi/2,1280),('oblique',.5,False,.37,1280)]:
        # Exact text parsed by the sample is the independent ODE initial condition.
        state=kepler(0,e,rev,angle)
        state=[0.0 if abs(v)<1e-15 else v for v in state]
        cases[name]={'input':[1,*state,2*math.pi/steps,steps], 'analytic':{'e':e,'reversed':rev,'angle':angle}}
    cases['general']={'input':[2,1.2,.4,-.3,.9,.002,1000]}
    cases['maximum_steps']={'input':[1,1,0,0,1,.001,4096]}
    cases['initial_collision']={'input':[1,.0000005,0,0,0,.001,1], 'exit':2,'status':'collision_threshold'}
    cases['crossing_collision']={'input':[.000000001,.001,0,-1,0,.001,1], 'exit':2,'status':'collision_threshold'}
    invalid={'zero_radius':'1 0 0 0 1 0.001 10','negative_mu':'-1 1 0 0 1 0.001 10','zero_mu':'0 1 0 0 1 0.001 10','zero_dt':'1 1 0 0 1 0 10','negative_dt':'1 1 0 0 1 -0.001 10','overflow_steps':'1 1 0 0 1 0.001 4097','integer_overflow':'1 1 0 0 1 0.001 2147483648','fractional_steps':'1 1 0 0 1 0.001 1.5','zero_steps':'1 1 0 0 1 0.001 0','nonfinite_nan':'1 NaN 0 0 1 0.001 10','nonfinite_inf':'1 1 0 0 Inf 0.001 10','coordinate_bound':'1 1000001 0 0 1 0.001 10','too_long':'9'*513,'extra':'1 1 0 0 1 0.001 10 5','trailing':'1 1 0 0 1 0.001 10\njunk','empty':'','missing':'1 1 0','malformed':'1 1..0 0 0 1 0.001 10'}
    for name,record in cases.items():
        text=' '.join(format(v,'.17f').rstrip('0').rstrip('.') if isinstance(v,float) else str(v) for v in record['input'])+'\n'
        (ROOT/'data'/f'{name}.stdin').write_text(text)
        values=[float(x) for x in text.split()]; mu,x,y,vx,vy,dt,n=values
        if record.get('exit'): continue
        times=[dt*k for k in range(int(n)+1)]
        def derivative(t,z):
            r=math.hypot(z[0],z[1]); factor=-mu/r**3
            return [z[2],z[3],factor*z[0],factor*z[1]]
        ode=solve_ivp(derivative,[0,times[-1]],[x,y,vx,vy],method='DOP853',rtol=2.3e-14,atol=2e-15,t_eval=times)
        assert ode.success
        record['ode']=[list(map(float,row)) for row in ode.y.T]
        if 'analytic' in record:
            a=record['analytic']; record['kepler']=[kepler(t,a['e'],a['reversed'],a['angle']) for t in times]
            assert max(math.dist(p,q) for p,q in zip(record['ode'],record['kepler']))<1e-10
        record['exit']=0; record['status']='complete'
    for name,text in invalid.items():
        (ROOT/'data'/f'{name}.stdin').write_text(text+'\n')
        cases[name]={'exit':64 if name in {'nonfinite_nan','nonfinite_inf','too_long','extra','trailing','empty','missing','malformed'} else 65,'status':'invalid'}
    oracle={'method':'analytic Kepler plus independent DOP853; rtol=2.3e-14 atol=2e-15','scipy':scipy.__version__,'position_tolerance':{'circular':1e-4,'eccentric':.0015,'general':.0001},'cases':cases}
    (ROOT/'validation/oracle.json').write_text(json.dumps(oracle,separators=(',',':'))+'\n')
    paths=[ROOT/'validation/reference.py',ROOT/'validation/oracle.json',*sorted((ROOT/'data').glob('*.stdin'))]
    (ROOT/'validation/frozen.sha256').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(ROOT))+'\n' for p in paths))
    print('Frozen',len(cases),'cases; analytic/ODE agreement <1e-10')
if __name__=='__main__':freeze()
