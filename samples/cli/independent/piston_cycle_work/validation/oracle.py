"""Freeze analytic rational results before executing Omega; no output learns them."""
from fractions import Fraction as F
import json
from pathlib import Path
root = Path(__file__).resolve().parent
# v0,p0,v1,p1,n,hold -- isochoric paths interpolate imposed pressure.
forward = [(1,4,2,4,0,False),(2,4,2,1,0,True),(2,1,1,1,0,False),(1,1,1,4,0,True)]
reverse = [(1,4,1,1,0,True),(1,1,2,1,0,False),(2,1,2,4,0,True),(2,4,1,4,0,False)]
cases = {0:forward,1:reverse,2:[(1,12,3,0,2,False)],3:[(1,8,2,0,3,False)],4:[(1,2,3,0,-1,False)],8:[(1,4,1,0,2,False)],9:[(3,F(4,3),1,0,2,False)]}
result = {'internal_absolute_tolerance':1e-9,'printed_absolute_tolerance':2e-6,'cases':{}}
for key, stages in cases.items():
    rows=[]; works=[]
    for index,(v0,p0,v1,p1,n,hold) in enumerate(stages):
        v0,p0,v1,p1 = map(F,(v0,p0,v1,p1))
        pressure=lambda v:p0*(v0/v)**n
        if not hold:p1=pressure(v1)
        middle=(v0+v1)/2
        for point,v,p in [(0,v0,p0),(1,middle,(p0+p1)/2 if hold else pressure(middle)),(2,v1,p1)]:
            rows.append([index,point,float(v),float(p)])
        work=F(0) if hold else (p1*v1-p0*v0)/(1-n)
        works.append(float(work))
    result['cases'][str(key)]={'rows':rows,'works':works,'balance':[sum(w for w in works if w>=0),sum(w for w in works if w<0),sum(works)],'closure':'closed_cycle' if key in (0,1) else 'open_stage','exit':0}
result['rejections']={'5':[65,'invalid_stage'],'6':[66,'discontinuous_boundary'],'7':[65,'invalid_stage']}
(root/'expected.json').write_text(json.dumps(result,indent=2)+'\n')
