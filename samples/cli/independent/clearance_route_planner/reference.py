#!/usr/bin/env python3
"""Freeze independent exact oracles; no Omega output is read by this program."""
import argparse
import hashlib
import itertools
import json
import math
from fractions import Fraction
from pathlib import Path
ROOT = Path(__file__).resolve().parent

def segment_distance(first, second, center):
    # Independent projection-coordinate implementation (Omega uses a cross product).
    direction = tuple(b - a for a, b in zip(first, second))
    length = sum(v * v for v in direction)
    projection = sum((c-a)*v for c,a,v in zip(center,first,direction))
    parameter = max(Fraction(0), min(Fraction(1), Fraction(projection, length))) if length else Fraction(0)
    closest = tuple(Fraction(a) + parameter*v for a,v in zip(first,direction))
    return sum((Fraction(c)-q)**2 for c,q in zip(center,closest))

def milli(value):
    return math.isqrt(value.numerator * 1000000 // value.denominator)

def edge(data, scene, radius, first, second):
    nodes = data['nodes']
    bounds = data['bounds']
    wall = min(coordinate - bounds[axis*2] for node in (first,second) for axis,coordinate in enumerate(nodes[node]))
    wall = min(wall, min(bounds[axis*2+1]-coordinate for node in (first,second) for axis,coordinate in enumerate(nodes[node])))
    clearance = wall*1000
    valid = wall >= radius
    for x,y,obstacle_radius in data['scenes'][scene]['obstacles']:
        distance = segment_distance(nodes[first],nodes[second],(x,y))
        valid &= distance >= (radius + obstacle_radius)**2
        clearance = min(clearance,milli(distance)-obstacle_radius*1000)
    weight = milli(Fraction(sum((a-b)**2 for a,b in zip(nodes[first],nodes[second]))))
    return valid,weight,clearance

def oracle(data, scene, radius):
    prefix = f'clearance_route_planner\nradius={radius}\nscene={scene}\n'
    for endpoint in (0,1):
        if not edge(data,scene,radius,endpoint,endpoint)[0]:
            clearance=edge(data,scene,0,endpoint,endpoint)[2]
            return 2, prefix+f'status=endpoint_blocked\nnode={endpoint}\nminimum_clearance_milli={clearance}\n', [], None, None
    feasible=[]
    for count in range(5):
        for interior in itertools.permutations(range(2,6),count):
            path=(0,*interior,1)
            segments=[edge(data,scene,radius,a,b) for a,b in zip(path,path[1:])]
            if all(v[0] for v in segments):
                feasible.append((sum(v[1] for v in segments),path,min(v[2] for v in segments)))
    if feasible:
        cost,path,clearance=min(feasible)
        coordinates=';'.join(','.join(map(str,data['nodes'][node])) for node in path)
        return 0,prefix+f'status=route\ncost_milli={cost}\nminimum_clearance_milli={clearance}\ncoordinates={coordinates}\nvalidated=1\n',list(path),cost,clearance
    reachable={0}
    while True:
        extended=reachable | {b for a in reachable for b in range(6) if a!=b and edge(data,scene,radius,a,b)[0]}
        if extended == reachable:break
        reachable=extended
    assert 1 not in reachable
    assert all(not edge(data,scene,radius,a,b)[0] for a in reachable for b in set(range(6))-reachable)
    mask=sum(1<<node for node in reachable)
    return 2,prefix+f'status=infeasible\nreachable_mask={mask}\ncut_validated=1\n',[],None,None

def freeze():
    data=json.loads((ROOT/'data/scenes.json').read_text())
    cases=[]
    for scene in range(2):
        for radius in range(6):
            exit_code,output,path,cost,clearance=oracle(data,scene,radius)
            cases.append(dict(name=f'r{radius}-s{scene}',input=f'{radius} {scene}\n',exit=exit_code,stdout=output,path=path,cost=cost,clearance=clearance))
    for name,request in [('empty',''),('negative','-1 0\n'),('large','6 0\n'),('scene','2 2\n'),('letter','x 0\n'),('extra','2 0 x\n'),('full','12345678\n'),('eof','2 0'),('crlf','2 0\r\n'),('spacing','2  0\n')]:
        cases.append(dict(name=name,input=request,exit=64,stdout='invalid_input: expected radius(0..5) space scene(0..1) LF\n'))
    result=dict(oracle='projection-coordinate Fractions + exhaustive 65 simple paths per valid endpoint pair',clearance_error='0 <= exact clearance - reported/1000 < 0.001; exact feasibility; exact quantized cost',scene_sha256=hashlib.sha256((ROOT/'data/scenes.json').read_bytes()).hexdigest(),cases=cases)
    return json.dumps(result,indent=2)+'\n'

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--freeze',action='store_true');options=parser.parse_args()
    expected=ROOT/'data/expected.json'
    content=freeze()
    if options.freeze:
        if expected.exists():raise SystemExit('refusing to replace frozen oracle')
        expected.write_text(content)
    else:
        assert expected.read_text()==content,'frozen oracle differs'
    print('independent exact reference: 12 domain variants + 10 malformed inputs')
