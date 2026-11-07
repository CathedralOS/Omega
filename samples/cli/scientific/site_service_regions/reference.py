"""Independent exact oracle: enumerate feasible line intersections, not clipping.
Run freeze only before the first Omega execution. Validation never changes it.
"""
from fractions import Fraction as F
from itertools import combinations
from pathlib import Path
import json, math, hashlib, sys
ROOT=Path(__file__).resolve().parent

def read(path):
    values=list(map(int,path.read_text().split())); cursor=0
    count=values[cursor];cursor+=1
    boundary=[tuple(values[cursor+2*i:cursor+2*i+2]) for i in range(count)];cursor+=2*count
    count=values[cursor];cursor+=1
    sites=[tuple(values[cursor+2*i:cursor+2*i+2]) for i in range(count)]
    return boundary,sites

def cross(first,second):return first[0]*second[1]-first[1]*second[0]
def area(vertices):return sum(cross(vertices[i],vertices[(i+1)%len(vertices)]) for i in range(len(vertices)))/F(2)
def oracle(boundary,sites):
    walls=[]
    for index,start in enumerate(boundary):
        end=boundary[(index+1)%len(boundary)]; dx=end[0]-start[0];dy=end[1]-start[1]
        walls.append((dy,-dx,dy*start[0]-dx*start[1]))
    cells=[]
    for owner,(x,y) in enumerate(sites):
        lines=walls+[(2*(u-x),2*(v-y),u*u+v*v-x*x-y*y) for index,(u,v) in enumerate(sites) if index!=owner]
        vertices=set()
        for (a,b,c),(d,e,f) in combinations(lines,2):
            determinant=a*e-b*d
            if not determinant:continue
            point=(F(c*e-b*f,determinant),F(a*f-c*d,determinant))
            if all(A*point[0]+B*point[1]<=C for A,B,C in lines): vertices.add(point)
        center=tuple(sum(p[axis] for p in vertices)/len(vertices) for axis in (0,1))
        vertices=sorted(vertices,key=lambda p:math.atan2(float(p[1]-center[1]),float(p[0]-center[0])))
        adjacent=[]
        for rival,(u,v) in enumerate(sites):
            if rival==owner:continue
            bisector=(2*(u-x),2*(v-y),u*u+v*v-x*x-y*y)
            on=[p for p in vertices if bisector[0]*p[0]+bisector[1]*p[1]==bisector[2]]
            if len(on)>=2:adjacent.append(rival)
        cells.append({'vertices':[[str(x),str(y)] for x,y in vertices], 'area':str(area(vertices)), 'adjacent':adjacent})
    assert sum(F(cell['area']) for cell in cells)==area(boundary)
    return {'cells':cells,'area':str(area(boundary))}

def freeze():
    output={'tolerances':{'coordinate':0.000002,'area':0.000004,'distance_squared':0.00002,'partition_area':0.00002},'method':'exact feasible pairwise line intersections; rational shoelace', 'fixtures':{}}
    for path in sorted((ROOT/'data').glob('*.txt')):
        if path.name.startswith('invalid_'):continue
        boundary,sites=read(path); result=oracle(boundary,sites)
        result['input_sha256']=hashlib.sha256(path.read_bytes()).hexdigest();output['fixtures'][path.stem]=result
    (ROOT/'data'/'expected.json').write_text(json.dumps(output,indent=2)+'\n')
    print(hashlib.sha256((ROOT/'data'/'expected.json').read_bytes()).hexdigest())
if __name__=='__main__':
    assert sys.argv[1:]==['freeze'], 'only explicit freeze is supported'
    freeze()
