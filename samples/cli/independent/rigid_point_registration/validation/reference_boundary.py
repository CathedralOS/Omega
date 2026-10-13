"""Freeze numerical-envelope boundary cases before application execution."""
import json,hashlib
import numpy as np
from reference import ROOT,reference
p=ROOT/'data/oracle_boundary.json';assert not p.exists(),'refuse to overwrite boundary oracle'
cases={}
def add(name,source,target,weights,status=0):
    payload=str(len(source))+' '+' '.join(format(float(x),'.8f').rstrip('0').rstrip('.') or '0' for row in zip(source,target,weights) for x in [*row[0],*row[1],row[2]])+'\n'
    (ROOT/'data'/f'{name}.txt').write_text(payload)
    cases[name]=dict(input_sha256=hashlib.sha256(payload.encode()).hexdigest(),exit=status,expected=reference(source,target,weights) if status==0 else None)
for height,status in [(0.000001,2),(0.000002,0)]:
    s=np.array([[0.,0,0],[1,0,0],[0,height,0]])
    add('rank_below' if status else 'rank_above',s,s,np.ones(3),status)
s=np.array([[-800.,0,0],[0,800,0],[0,0,-800],[800,0,0],[0,-800,0],[0,0,800]])
r=np.diag([-1.,-1.,1.]);t=s@r.T+np.array([-100,100,-50])
add('large_exact',s,t,np.full(6,1000.))
add('tiny_weights',s,t,np.full(6,0.00000001))
add('weight_scale',s,t+np.array([[.01,-.02,.03],[-.03,.01,0],[.02,.01,-.01],[.01,0,-.02],[-.02,.03,.01],[.02,-.01,.02]]),np.array([1000,200,300,400,150,50]))
p.write_text(json.dumps(dict(method='independent weighted Kabsch/SVD; rank threshold and input envelope',cases=cases),indent=2)+'\n');print(hashlib.sha256(p.read_bytes()).hexdigest())
