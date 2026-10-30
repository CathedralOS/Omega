"""A finite effect set must converge even when recursive discovery order changes."""
import json
direct=[[],[],[],[3],[4],[5]]
calls=[[1],[2,5,1],[0,4,3],[5],[2,1],[0,4,1]]
def solve(canonical):
    work=[x[:] for x in direct]; seen={}; history=[]
    for iteration in range(40):
        key=tuple(tuple(x) for x in work)
        if key in seen:
            return dict(period=iteration-seen[key],iterations=iteration,work=work,history=history)
        seen[key]=iteration; history.append(work)
        work=[x[:] for x in work]
        for i in range(len(work)):
            result=direct[i][:]
            for callee in calls[i]:
                for target in work[callee]:
                    if target not in result: result.append(target)
            work[i]=sorted(result) if canonical else result
    raise AssertionError('model failed its bound')
before=solve(False); after=solve(True)
assert before['period']==2 and after['period']==1
# Independent graph reachability supplies the semantic set oracle.
for start,actual in enumerate(after['work']):
    todo=[start]; visited=set(); expected=set()
    while todo:
        current=todo.pop()
        if current in visited:continue
        visited.add(current); expected.update(direct[current]); todo.extend(calls[current])
    assert set(actual)==expected==set(before['work'][start])
print(json.dumps(dict(direct=direct,calls=calls,before=before,after=after),indent=2))
