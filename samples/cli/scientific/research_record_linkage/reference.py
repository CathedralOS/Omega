"""Independent exhaustive oracle: subsets of left records and right permutations.
No base-four assignment decoding or streaming consensus used by the Omega app.
All comparisons are exact integers (absolute and relative tolerance zero).
"""
import itertools
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parent

def reference(tokens):
    threshold = tokens[0]
    records = [tuple(tokens[1+4*i:5+4*i]) for i in range(6)]
    truth = tokens[25:28]
    def score(a,b):
        if a[0] and b[0] and a[0] != b[0]:
            return 0
        return sum(weight for x,y,weight in zip(a,b,(12,6,3,1)) if x and x==y)
    matrix = [[score(a,b) for b in records[3:]] for a in records[:3]]
    admitted = {(i,j): matrix[i][j] for i in range(3) for j in range(3) if matrix[i][j] >= threshold}
    matchings = []
    for size in range(4):
        for selected in itertools.combinations(range(3),size):
            for targets in itertools.permutations(range(3),size):
                pairs = tuple(zip(selected,targets))
                if all(pair in admitted for pair in pairs):
                    choices = [0,0,0]
                    for i,j in pairs: choices[i]=j+1
                    matchings.append((sum(admitted[pair] for pair in pairs),tuple(choices)))
    best = max(total for total,_ in matchings)
    winners = [choices for total,choices in matchings if total==best]
    sets = [set(choices[i] for choices in winners) for i in range(3)]
    consensus = [next(iter(s)) if len(s)==1 else 4 for s in sets]
    tp = sum(0<c<4 and c==t for c,t in zip(consensus,truth))
    fp = sum(0<c<4 and c!=t for c,t in zip(consensus,truth))
    fn = sum(t>0 and c!=t for c,t in zip(consensus,truth))
    result = dict(best=best,optima=len(winners),edges=len(admitted),
        conflicts=sum(bool(a[0] and b[0] and a[0]!=b[0]) for a in records[:3] for b in records[3:]),
        linked=sum(0<c<4 for c in consensus),ambiguous=consensus.count(4),unmatched=consensus.count(0),tp=tp,fp=fp,fn=fn)
    result.update({f"left{i+1}":c for i,c in enumerate(consensus)})
    return result, matrix, winners

def render(result):
    return ''.join(f"{key}={value}\n" for key,value in result.items())

if __name__ == '__main__':
    frozen = json.loads((ROOT/'data/frozen.json').read_text())
    for name,case in frozen['cases'].items():
        tokens = list(map(int,(ROOT/'data'/f'{name}.txt').read_text().split()))
        result,matrix,winners = reference(tokens)
        assert result==case['result'],name
        assert matrix==case['matrix'],name
        assert [list(w) for w in winners]==case['optimal_assignments'],name
        assert render(result)==(ROOT/'data'/f'{name}.stdout').read_text(),name
    print(f"frozen independent oracle: {len(frozen['cases'])} cases verified")
