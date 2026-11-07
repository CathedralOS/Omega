from pathlib import Path
from fractions import Fraction as F
from itertools import combinations
import json,hashlib
p=Path(__file__).resolve().parent
cases={
 'rectangle':(10,[(1,0,4000000),(0,1,3000000),(-1,0,0),(0,-1,0)]),
 'triangle':(10,[(-1,0,0),(0,-1,0),(1,1,6000000)]),
 'redundant':(10,[(1,0,4000000),(0,1,3000000),(-1,0,0),(0,-1,0),(1,1,100000000),(2,0,8000000)]),
 'inconsistent':(10,[(1,0,-1000000),(-1,0,0)]),
 'diamond':(10,[(1,1,2000000),(-1,1,2000000),(-1,-1,2000000),(1,-1,2000000)]),
 'near_boundary':(10,[(1,0,1000001),(-1,0,-1000000),(0,1,2000000),(0,-1,0)]),
 'window':(3,[(1,0,1000000)]),
 'no_constraints':(2,[]),
 'eight_edges':(10,[(1,0,3000000),(0,1,3000000),(-1,0,3000000),(0,-1,3000000),(1,1,4000000),(-1,1,4000000),(-1,-1,4000000),(1,-1,4000000)]),
 'scaled_triangle':(10,[(-2,0,0),(0,-3,0),(2,2,12000000)]),
}
def reference(bound,triples):
    lines=[(F(a),F(b),F(c,1000000)) for a,b,c in triples]+[(F(1),F(0),F(bound)),(-F(1),F(0),F(bound)),(F(0),F(1),F(bound)),(F(0),-F(1),F(bound))]
    points=set()
    for (a,b,c),(d,e,f) in combinations(lines,2):
        det=a*e-b*d
        if det:
            x=(c*e-b*f)/det;y=(a*f-c*d)/det
            if all(u*x+v*y<=w for u,v,w in lines):points.add((x,y))
    points=sorted(points)
    def cross(a,b,c):return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
    lower=[];upper=[]
    for v in points:
        while len(lower)>1 and cross(lower[-2],lower[-1],v)<=0:lower.pop()
        lower.append(v)
    for v in reversed(points):
        while len(upper)>1 and cross(upper[-2],upper[-1],v)<=0:upper.pop()
        upper.append(v)
    hull=lower[:-1]+upper[:-1] if len(points)>1 else points
    area=sum((hull[i][0]*hull[(i+1)%len(hull)][1]-hull[i][1]*hull[(i+1)%len(hull)][0] for i in range(len(hull))),F(0))/2
    active=[[j for j,(a,b,c) in enumerate(lines[:len(triples)]) if a*x+b*y==c] for x,y in hull]
    return dict(vertices=[[float(x),float(y)] for x,y in hull],area=float(area),area_exact=str(area),active=active,status='empty-in-window' if not hull else 'window-limited-unboundedness-undetermined' if any(abs(x)==bound or abs(y)==bound for x,y in hull) else 'bounded')

if __name__ == '__main__':
    for name,(bound,triples) in cases.items(): print(name,reference(bound,triples))
