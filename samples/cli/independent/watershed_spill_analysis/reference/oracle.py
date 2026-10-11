"""Independent exhaustive-level filling; separately specified deterministic drainage.
Freeze mode writes expected output before executing Omega. Verify never rewrites.
All arithmetic is integer; numerical tolerance is exactly zero.
"""
from pathlib import Path
import argparse, hashlib, json
ROOT = Path(__file__).resolve().parents[1]

def neighbors(cell):
    row, column = divmod(cell, 5)
    return [other for other in range(25) if abs(row-other//5)+abs(column-other%5)==1]

def boundary(cell):
    row, column = divmod(cell, 5)
    return row in (0,4) or column in (0,4)

def flooded(heights):
    filled = [None]*25
    # For each water elevation, exhaustively find its boundary-connected cells.
    for level in range(10):
        wet = {i for i in range(25) if boundary(i) and heights[i] <= level}
        while True:
            more = {j for i in wet for j in neighbors(i) if heights[j] <= level}
            if more <= wet: break
            wet |= more
        for i in wet:
            if filled[i] is None: filled[i] = level
    assert all(v is not None for v in filled)
    return filled

def drainage(heights):
    # Independent priority implementation to specify tie semantics for labels.
    import heapq
    costs=[99]*25; parents=list(range(25)); outlets=list(range(25)); done=set(); queue=[]
    for i in range(25):
        if boundary(i):
            costs[i]=heights[i]; heapq.heappush(queue,(costs[i],i))
    while queue:
        level, i=heapq.heappop(queue)
        if i in done: continue
        done.add(i)
        for j in neighbors(i):
            proposal=max(level,heights[j])
            if j not in done and proposal < costs[j]:
                costs[j]=proposal; parents[j]=i; outlets[j]=outlets[i]
                heapq.heappush(queue,(proposal,j))
    assert costs == flooded(heights)
    # Independently walk every drainage chain and check the selected root.
    for i in range(25):
        seen=set(); current=i
        while parents[current] != current:
            assert current not in seen; seen.add(current)
            following=parents[current]
            assert following in neighbors(current) and costs[following] <= costs[current]
            current=following
        assert boundary(current) and current == outlets[i]
    return parents,outlets

def expected(text):
    heights=[int(c) for c in text if c not in ' \t\r\n']
    assert len(heights)==25
    filled=flooded(heights); parents,outlets=drainage(heights)
    volumes=[sum(filled[i]-heights[i] for i in range(25) if outlets[i]==outlet) for outlet in range(25)]
    total=sum(filled[i]-heights[i] for i in range(25))
    assert sum(volumes)==total
    def grid(values): return '\n'.join(' '.join(map(str,values[i:i+5])) for i in range(0,25,5))+'\n'
    return ''.join(name+'\n'+grid(values) for name,values in [('filled',filled),('outlets',outlets),('parents',parents),('volumes',volumes)])+'total\n'+str(total)+'\n'

def freeze():
    manifest={'strategy':'integer-level exhaustive boundary flooding; heap drainage; independent depth sums','tolerance':0,'cases':{}}
    for fixture in sorted((ROOT/'fixtures').glob('*.txt')):
        if fixture.name.endswith('.expected.txt'): continue
        invalid=fixture.stem.startswith('invalid_')
        output='ERROR: expected exactly 25 elevation digits (0-9)\n' if invalid else expected(fixture.read_text())
        path=fixture.with_suffix('.expected.txt'); path.write_text(output)
        manifest['cases'][fixture.stem]={'input_sha256':hashlib.sha256(fixture.read_bytes()).hexdigest(),'stdout_sha256':hashlib.sha256(output.encode()).hexdigest(),'exit':2 if invalid else 0}
    (ROOT/'reference/frozen.json').write_text(json.dumps(manifest,indent=2)+'\n')

if __name__=='__main__':
    parser=argparse.ArgumentParser(); parser.add_argument('--freeze',action='store_true'); options=parser.parse_args()
    if options.freeze: freeze()
    else:
        manifest=json.loads((ROOT/'reference/frozen.json').read_text())
        for name,case in manifest['cases'].items():
            fixture=ROOT/'fixtures'/f'{name}.txt'; output=fixture.with_suffix('.expected.txt')
            assert hashlib.sha256(fixture.read_bytes()).hexdigest()==case['input_sha256']
            assert hashlib.sha256(output.read_bytes()).hexdigest()==case['stdout_sha256']
            if not name.startswith('invalid_'): assert output.read_text()==expected(fixture.read_text())
        print(f"frozen oracles verified: {len(manifest['cases'])}; tolerance 0")
