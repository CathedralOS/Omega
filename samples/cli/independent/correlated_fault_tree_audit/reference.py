from pathlib import Path
import itertools,json,hashlib
p=Path('samples/cli/independent/correlated_fault_tree_audit')
# Independent set algebra: no masks in the failure evaluator, no reuse of Omega.
def events(mask): return frozenset(i for i in range(3) if mask & (1<<i))
def oracle(terms, weights):
    clauses=[events(t) for t in terms]
    def failed(s): return any(t.issubset(s) for t in clauses)
    outcomes=list(map(events,range(8)))
    cuts=[i for i,s in enumerate(outcomes) if failed(s) and not any(failed(t) for t in outcomes if t<s)]
    failure=sum(w for s,w in zip(outcomes,weights) if failed(s))
    lines=[f'failure_ppm={failure}/1000000',*[f'cut_mask={i}' for i in cuts]]
    for event in range(3):
        repaired=sum(w for s,w in zip(outcomes,weights) if failed(s-{event}))
        forced=sum(w for s,w in zip(outcomes,weights) if failed(s|{event}))
        marginal=sum(w for s,w in zip(outcomes,weights) if event in s)
        lines.append(f'event={event} marginal_ppm={marginal} repaired_ppm={repaired} reduction_ppm={failure-repaired} forced_ppm={forced}')
    return '\n'.join(lines+['AUDIT COMPLETE'])+'\n'
# Verification only; the application never calls this reference.
if __name__ == '__main__':
    for path in (p/'data').glob('*.in'):
        if path.stem.startswith('invalid_'): continue
        values=list(map(int,path.read_text().split()))
        expected=oracle(values[1:1+values[0]],values[5:])
        assert expected == path.with_suffix('.out').read_text(), path
    print('Frozen set-algebra oracles verified; tolerance=0')
