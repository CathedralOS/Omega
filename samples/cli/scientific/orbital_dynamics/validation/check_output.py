"""Numerically verify actual captured streams against immutable independent references."""
import csv, hashlib, json, math
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

def frozen():
    for manifest in ['validation/frozen.sha256','validation/extra_frozen.sha256','validation/parser_frozen.sha256']:
        for line in (ROOT/manifest).read_text().splitlines():
            expected,name=line.split('  ',1)
            assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==expected,name
    oracle=json.loads((ROOT/'validation/oracle.json').read_text())
    for name in ['validation/extra_oracle.json','validation/parser_oracle.json']:
        extra=json.loads((ROOT/name).read_text())
        assert not set(oracle['cases']) & set(extra['cases'])
        oracle['cases'].update(extra['cases'])
    return oracle

def check(name,output,exit_code):
    oracle=frozen(); case=oracle['cases'][name]; text=Path(output).read_text()
    assert exit_code==case['exit'],(name,exit_code,case['exit'],text[:300])
    if case['status']=='invalid':
        expected='ERROR syntax:' if exit_code==64 else 'ERROR range:'
        assert text.startswith(expected) and len(text.splitlines())==1,(name,text)
        return {}
    rows=list(csv.reader(text.splitlines()))
    assert rows[-1]==['status',case['status']],(name,rows[-1])
    if exit_code==2:
        assert not any(row[0]=='periapsis' for row in rows)
        if name=='initial_collision': assert len(rows)==1
        if name in {'crossing_collision','interior_collision'}: assert len(rows)==3
        return {}
    expected_header=['checkpoint','step','time','x','y','vx','vy','energy','angular_momentum','energy_drift','angular_momentum_drift']
    assert rows[0]==expected_header
    checkpoints=[r for r in rows[1:-1] if r[0]=='checkpoint']
    events=[r for r in rows[1:-1] if r[0]=='periapsis']
    assert len(rows)==len(checkpoints)+len(events)+2
    assert len(checkpoints)==len(case['ode'])
    mu,*_=case['input'];dt=case['input'][5]
    initial=[float(x) for x in checkpoints[0][2:]]
    max_error=0; max_momentum=0; max_energy=0; max_independent_momentum=0
    for k,(row,reference) in enumerate(zip(checkpoints,case['ode'])):
        assert len(row)==11 and int(row[1])==k
        time,x,y,vx,vy,energy,momentum,energy_drift,momentum_drift=map(float,row[2:])
        assert all(math.isfinite(z) for z in [time,x,y,vx,vy,energy,momentum,energy_drift,momentum_drift])
        assert abs(time-k*dt)<=1e-12
        error=math.dist([x,y],reference[:2]);max_error=max(max_error,error)
        category='eccentric' if case.get('analytic',{}).get('e',0)>0 else ('general' if name=='general' else 'circular')
        tolerance=oracle['position_tolerance'][category] * (4 if name=='circular_coarse' else 1)
        assert error<=tolerance,(name,k,error)
        assert math.dist([vx,vy],reference[2:])<=5*tolerance,(name,k,'velocity')
        assert abs(energy-(.5*(vx*vx+vy*vy)-mu/math.hypot(x,y)))<=2e-13
        assert abs(momentum-(x*vy-y*vx))<=2e-13
        assert abs(energy_drift-(energy-initial[5]))<=2e-13
        assert abs(momentum_drift-(momentum-initial[6]))<=2e-13
        max_momentum=max(max_momentum,abs(momentum_drift))
        max_independent_momentum=max(max_independent_momentum,abs(x*vy-y*vx-initial[6]))
        max_energy=max(max_energy,abs(energy_drift))
    assert max_momentum<=1e-10,(name,'angular momentum',max_momentum)
    assert max_independent_momentum<=1e-10,(name,'independent angular momentum',max_independent_momentum)
    assert max_energy<=.0005,(name,'energy drift',max_energy)
    previous_event=-1.0
    for event in events:
        assert len(event)==2 and math.isfinite(float(event[1])),(name,event)
        event_time=float(event[1])
        assert previous_event < event_time <= (len(checkpoints)-1)*dt+1e-12 and event_time >= 0
        previous_event=event_time
    if case.get('analytic',{}).get('e',0)>0:
        assert len(events)==1,(name,events)
        assert abs(float(events[0][1])-math.pi)<=.001,(name,'periapsis',events)
    last=list(map(float,checkpoints[-1][3:7]))
    final_error=math.dist(last[:2],case['ode'][-1][:2])
    return {'maximum_position_error':max_error,'final_position_error':final_error,'maximum_independent_angular_momentum_drift':max_independent_momentum,'maximum_energy_drift':max_energy,'maximum_angular_momentum_drift':max_momentum,'periapsis_times':[float(e[1]) for e in events]}

def directory(path):
    path=Path(path);oracle=frozen();records=list(csv.DictReader((path/'records.tsv').open(),delimiter='\t'))
    assert len(records)==len(oracle['cases']) and {r['case'] for r in records}==set(oracle['cases'])
    results={}
    for row in records:
        name=row['case']; results[name]=check(name,path/(name+'.stdout'),int(row['exit']))
        assert (path/(name+'.stderr')).read_bytes()==b'',name
    for coarse,fine in [('circular_coarse','circular'),('circular','circular_half'),('eccentric','eccentric_half')]:
        ratio=results[coarse]['final_position_error']/results[fine]['final_position_error']
        assert 3.7<ratio<4.3,(coarse,fine,'second-order convergence',ratio)
        results[coarse]['halving_ratio']=ratio
    (path/'verification.json').write_text(json.dumps(results,indent=2)+'\n')
    print('Verified',len(results),'numerical and refusal cases in',path)
    return results
if __name__=='__main__':
    import sys
    directory(sys.argv[1])
