"""Validate captured application output against the pre-execution exact oracle."""
from pathlib import Path
from fractions import Fraction
import json,hashlib,math,sys
from reference import read,area
ROOT=Path(__file__).resolve().parent
EXPECTED_SHA='d7f9565bd692122304bbb18814334fccbe560ba1e019047daa78b03eda82e6b2'

def verify(name,path):
    golden_path=ROOT/'data'/'expected.json'
    assert hashlib.sha256(golden_path.read_bytes()).hexdigest()==EXPECTED_SHA,'frozen oracle changed'
    golden=json.loads(golden_path.read_text()); expected=golden['fixtures'][name]; tolerance=golden['tolerances']
    fixture=ROOT/'data'/f'{name}.txt'
    assert hashlib.sha256(fixture.read_bytes()).hexdigest()==expected['input_sha256'],'fixture changed'
    boundary,sites=read(fixture)
    lines=path.read_text().splitlines();assert lines.pop(0)=='SERVICE_REGIONS micro_units=1000000'
    cells=[];total=None
    for line in lines:
        tokens=line.split()
        if tokens[0]=='CELL':
            assert tokens[2]=='AREA';assert int(tokens[1])==len(cells)
            cells.append({'area':int(tokens[3])/1e6,'vertices':[],'adjacent':[],'nearest':False})
        elif tokens[0]=='VERTEX':cells[-1]['vertices'].append(tuple(int(v)/1e6 for v in tokens[1:]))
        elif tokens[0]=='ADJACENT':cells[-1]['adjacent'].append(int(tokens[1]))
        elif tokens[0]=='NEAREST':assert tokens==['NEAREST','PASS'];cells[-1]['nearest']=True
        elif tokens[0]=='TOTAL_AREA':total=int(tokens[1])/1e6
        else:assert tokens==['PARTITION','PASS']
    assert lines[-1]=='PARTITION PASS' and len(cells)==len(sites)
    summed=0.0
    for owner,(cell,wanted) in enumerate(zip(cells,expected['cells'])):
        assert cell['nearest']; assert cell['adjacent']==wanted['adjacent']
        assert abs(cell['area']-float(Fraction(wanted['area'])))<=tolerance['area']
        vertices=cell['vertices']; unique=[]
        for point in vertices:
            if not any(math.dist(point,prior)<=tolerance['coordinate'] for prior in unique):unique.append(point)
        target=[tuple(float(Fraction(v)) for v in point) for point in wanted['vertices']]
        assert len(unique)==len(target),(owner,unique,target)
        assert all(any(math.dist(point,other)<=tolerance['coordinate'] for other in unique) for point in target)
        measured=float(area(vertices)); assert measured>0 and abs(measured-cell['area'])<=tolerance['partition_area']
        summed+=measured
        # Independent checks use squared distances rather than clipping signs.
        center=tuple(sum(p[axis] for p in unique)/len(unique) for axis in (0,1))
        probes=vertices+[center]+[tuple((vertices[i][axis]+vertices[(i+1)%len(vertices)][axis])/2 for axis in (0,1)) for i in range(len(vertices))]
        for point in probes:
            own=sum((point[axis]-sites[owner][axis])**2 for axis in (0,1))
            assert all(own<=sum((point[axis]-rival[axis])**2 for axis in (0,1))+tolerance['distance_squared'] for rival in sites)
            for index,start in enumerate(boundary):
                end=boundary[(index+1)%len(boundary)]
                cross=(end[0]-start[0])*(point[1]-start[1])-(end[1]-start[1])*(point[0]-start[0])
                assert cross>=-tolerance['distance_squared']
        for rival in cell['adjacent']:assert owner in cells[rival]['adjacent'] and owner!=rival
    assert total is not None
    assert abs(total-float(Fraction(expected['area'])))<=tolerance['partition_area']
    assert abs(summed-total)<=tolerance['partition_area']
    print(f'{name}: vertices, areas, adjacency, distance probes and partition PASS')
if __name__=='__main__': verify(sys.argv[1],Path(sys.argv[2]))
