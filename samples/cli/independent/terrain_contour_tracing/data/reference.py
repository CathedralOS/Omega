from fractions import Fraction
from collections import defaultdict
import json
from pathlib import Path
SCALE = 2520

def canonical(path):
    if path[0] != path[-1]: return min(path, path[::-1])
    ring = path[:-1]
    choices = []
    for orientation in (ring, ring[::-1]):
        for shift in range(len(ring)):
            candidate = orientation[shift:] + orientation[:shift]
            choices.append(candidate + [candidate[0]])
    return min(choices)

def reference(record):
    text=record.rstrip('\n')
    if len(text)!=11 or not text.isascii() or not text.isdigit() or text[1] not in '01':
        return {'exit':64,'stdout':'INVALID terrain record\n'}
    level, policy = int(text[0]), int(text[1])
    heights = list(map(int,text[2:]))
    if level in heights: return {'exit':64,'stdout':'INVALID terrain record\n'}
    edges=[]
    for row in range(3):
        for column in range(2): edges.append((row*3+column,row*3+column+1))
    for row in range(2):
        for column in range(3): edges.append((row*3+column,(row+1)*3+column))
    coordinates={}
    for identity,(a,b) in enumerate(edges):
        if (heights[a]>level)==(heights[b]>level): continue
        fraction=Fraction(level-heights[a],heights[b]-heights[a])
        x=Fraction(a%3)+fraction*(b%3-a%3)
        y=Fraction(a//3)+fraction*(b//3-a//3)
        assert (x*SCALE).denominator==(y*SCALE).denominator==1
        coordinates[identity]=[int(x*SCALE),int(y*SCALE)]
    segments=[]; saddles=0
    for row in range(2):
        for column in range(2):
            corners=[row*3+column,row*3+column+1,(row+1)*3+column+1,(row+1)*3+column]
            cell_edges=[row*2+column,6+row*3+column+1,(row+1)*2+column,6+row*3+column]
            crossings=[edge for edge in cell_edges if edge in coordinates]
            if len(crossings)==2: pairs=[crossings]
            elif len(crossings)==4:
                saddles+=1
                # Separate high corners for policy0, separate low for policy1.
                pairs=[[cell_edges[(index-1)%4],cell_edges[index]]
                       for index,corner in enumerate(corners)
                       if (heights[corner]>level)==(policy==0)]
            else: assert not crossings; pairs=[]
            segments.extend([row*2+column,*sorted(pair)] for pair in pairs)
    adjacency=defaultdict(list)
    for index,(_,a,b) in enumerate(segments):
        adjacency[a].append((index,b));adjacency[b].append((index,a))
    assert all(len(v)<=2 for v in adjacency.values())
    used=set(); paths=[]
    endpoints=sorted(v for v in adjacency if len(adjacency[v])==1)
    for origin in endpoints+sorted(adjacency):
        remaining=[(i,n) for i,n in adjacency[origin] if i not in used]
        if not remaining:continue
        path=[origin];current=origin
        while True:
            remaining=[(i,n) for i,n in adjacency[current] if i not in used]
            if not remaining:break
            index,current=min(remaining);used.add(index);path.append(current)
            if current==origin:break
        paths.append(canonical(path))
    assert len(used)==len(segments)
    return {'exit':0,'segments':sorted(segments),'paths':sorted(paths),
            'coordinates':{str(k):v for k,v in coordinates.items()},
            'topology':[len(segments),len(paths),sum(p[0]==p[-1] for p in paths),len(endpoints),saddles]}

if __name__=='__main__':
    root=Path(__file__).parent
    fixtures=json.loads((root/'fixtures.json').read_text())
    result={name:reference(record) for name,record in fixtures.items()}
    print(json.dumps(result,sort_keys=True,indent=2))
