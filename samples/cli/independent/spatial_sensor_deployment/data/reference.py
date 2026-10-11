"""Independent exhaustive oracle: combinations of sites, Cartesian demand assignments.
Integer squared Euclidean distance with closed radius; all checks are exact.
Never reads Omega output or learns expectations from execution.
"""
import itertools,json,pathlib
ROOT=pathlib.Path(__file__).resolve().parent

def solve(c):
    rows=[]; solutions=[]
    for flags in itertools.product((0,1),repeat=3):
        mask=sum(v*(2**i) for i,v in enumerate(flags))
        cost=sum(s[2] for s,v in zip(c['sites'],flags) if v)
        best=0; feasible=0; choice=0
        if cost<=c['budget']:
            for a in itertools.product(range(4),repeat=4):
                loads=[0]*3; score=0; valid=True
                for d,k in zip(c['demands'],a):
                    if not k:continue
                    s=c['sites'][k-1];loads[k-1]+=1
                    valid &= bool(flags[k-1]) and (s[0]-d[0])**2+(s[1]-d[1])**2<=s[4]**2
                    score+=d[2]
                valid &= all(loads[i]<=s[3] for i,s in enumerate(c['sites']))
                code=sum(k*4**i for i,k in enumerate(a))
                if valid:
                    feasible+=1
                    if score>best or score==best and code<choice:best,choice=score,code
            solutions.append((best,-cost,-mask,choice))
        rows.append([mask,cost,best,feasible,choice])
    win=max(solutions,key=lambda r:r[:3]);score,negative_cost,negative_mask,code=win
    ties=sum(r[0]==score and r[1]==negative_cost for r in solutions)
    return dict(id=c['id'],mask=-negative_mask,cost=-negative_cost,score=score,code=code,ties=ties,status=int(score>=c['minimum']),assignments=[(code//4**i)%4 for i in range(4)],certificates=sorted(rows))

def render(r):
    lines=['spatial_sensor_deployment v1',f"case={r['id']:04d}"]
    lines += ['certificate mask=%04d cost=%04d score=%04d feasible=%04d code=%04d'%tuple(row) for row in r['certificates']]
    lines += [f"placement mask={r['mask']:04d} cost={r['cost']:04d} score={r['score']:04d} code={r['code']:04d} ties={r['ties']:04d} status={r['status']:04d}"]
    lines += [f'demand={i:04d} site={k:04d}' for i,k in enumerate(r['assignments'])]
    return '\n'.join(lines)+'\n'
if __name__=='__main__':
    results=[solve(c) for c in json.loads((ROOT/'cases.json').read_text())]
    (ROOT/'expected.json').write_text(json.dumps(results,indent=2)+'\n')
    for r in results:(ROOT/f"expected-{r['id']}.txt").write_text(render(r))
