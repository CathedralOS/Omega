"""Independent oracle: enumerate chromosome-copy choices, not genotype vectors.
Freeze emits expected output before any Omega invocation. Exact integer oracles:
zero numerical tolerance; rational output intentionally not reduced.
"""
import itertools
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    'founder_unknown': [(0,0,7)],
    'founder_ambiguous': [(0,0,3)],
    'trio_unknown': [(0,0,7),(0,0,7),(1,2,7)],
    'trio_recessive': [(0,0,3),(0,0,3),(1,2,4)],
    'impossible_trio': [(0,0,1),(0,0,1),(1,2,4)],
    'shared_ancestry': [(0,0,7),(0,0,7),(1,2,7),(1,2,7),(3,4,4)],
    'shared_ambiguous': [(0,0,3),(0,0,6),(1,2,3),(1,2,6),(3,4,2)],
    'six_people': [(0,0,7),(0,0,7),(1,2,7),(1,2,7),(3,4,3),(1,5,6)],
    'two_contradictions': [(0,0,1),(0,0,1),(1,2,4),(1,2,2)],
}
INVALID = {
    'empty':'', 'zero':'0\n', 'too_many':'7\n', 'truncated':'2 0 0 7\n',
    'mask_zero':'1 0 0 0\n', 'mask_eight':'1 0 0 8\n',
    'self_parent':'2 0 0 7 1 2 7\n', 'one_parent':'2 0 0 7 1 0 7\n',
    'same_parents':'2 0 0 7 1 1 7\n', 'extra':'1 0 0 7 1\n',
    'multi_digit':'01 0 0 7\n', 'junk':'1 0 x 7\n', 'negative':'1 -1 0 7\n',
    'token_overflow':' '.join(['1']*20)+'\n',
}

def histories(rows, active):
    weights = Counter()
    # Every person has four equally likely chromosome choices, even homozygotes.
    for choices in itertools.product(range(4), repeat=len(rows)):
        copies = []
        for index, ((father,mother,mask), choice) in enumerate(zip(rows, choices)):
            if father == 0:
                pair = (choice // 2, choice % 2)
            else:
                pair = (copies[father-1][choice//2], copies[mother-1][choice%2])
            copies.append(pair)
            if active[index] and not mask & (1 << sum(pair)):
                break
        else:
            weights[tuple(map(sum, copies))] += 1
    return weights

def expected(rows):
    active = [True]*len(rows)
    weights = histories(rows, active)
    mass = sum(weights.values())
    lines = [f'SOLUTIONS {len(weights)}', f'EVIDENCE {mass}/{4**len(rows)}']
    if mass:
        for person in range(len(rows)):
            for genotype, label in enumerate(('AA','AB','BB')):
                numerator = sum(weight for assignment,weight in weights.items() if assignment[person]==genotype)
                if numerator:
                    lines.append(f'POSSIBLE {person+1} {label} {numerator}/{mass}')
        return '\n'.join(lines)+'\n', 0
    for person in range(len(rows)):
        active[person] = False
        if histories(rows, active):
            active[person] = True
    assert not histories(rows, active)
    for person, retained in enumerate(active):
        if retained:
            removed = active.copy()
            removed[person] = False
            assert histories(rows, removed), 'core must be irreducible'
            lines.append(f'CORE {person+1}')
    return '\n'.join(lines)+'\n', 2

def freeze():
    manifest = {'model':'biallelic, fair independent founder allele copies, fair parental transmissions, no mutation',
                'tolerance':0, 'cases':{}}
    for name,rows in CASES.items():
        input_text = str(len(rows))+'\n'+''.join(' '.join(map(str,row))+'\n' for row in rows)
        output, status = expected(rows)
        (ROOT/'fixtures'/f'{name}.stdin').write_text(input_text)
        (ROOT/'fixtures'/f'{name}.stdout').write_text(output)
        manifest['cases'][name]={'exit':status,'rows':rows}
    for name,input_text in INVALID.items():
        (ROOT/'fixtures'/f'invalid_{name}.stdin').write_text(input_text)
        (ROOT/'fixtures'/f'invalid_{name}.stdout').write_text('INVALID INPUT\n')
        manifest['cases'][f'invalid_{name}']={'exit':64}
    (ROOT/'validation'/'frozen.json').write_text(json.dumps(manifest,indent=2)+'\n')
    tiny = []
    for ancestry in ([(0,0)], [(0,0),(0,0)], [(0,0),(0,0),(1,2)]):
        for masks in itertools.product(range(1,8), repeat=len(ancestry)):
            rows = [(*parents,mask) for parents,mask in zip(ancestry,masks)]
            output,status = expected(rows)
            supplied = str(len(rows))+'\n'+''.join(' '.join(map(str,row))+'\n' for row in rows)
            tiny.append({'stdin':supplied,'stdout':output,'exit':status})
    (ROOT/'validation'/'tiny.json').write_text(json.dumps(tiny,indent=2)+'\n')

if __name__ == '__main__':
    freeze()
