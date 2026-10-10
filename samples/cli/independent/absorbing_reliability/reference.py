#!/usr/bin/env python3
"""Independent Fraction matrix reference. --freeze authors oracles before execution."""
import json
from fractions import Fraction as F
from pathlib import Path
import sys
ROOT = Path(__file__).resolve().parent
MATRICES = [
    [[2,1,1],[0,2,2],[0,0,4]],
    [[2,1,1],[1,2,1],[0,0,4]],
    [[3,1,0],[1,3,0],[0,0,4]],
    [[2,1,1],[0,2,2],[0,0,4]],
    [[0,4,0],[0,0,4],[0,0,4]],
    [[2,1,1],[0,2,2],[0,0,4]],
]
def expected(variant, horizon):
    matrix = [[F(x,4) for x in row] for row in MATRICES[variant]]
    distribution = [F(variant not in (3,5)), F(variant == 5), F(variant == 3)]
    lifetime = F(0)
    lines = ['step healthy degraded failed survival denominator']
    for step in range(horizon+1):
        denominator = 4**step
        assert sum(distribution) == 1 and min(distribution) >= 0
        survival = sum(distribution[:2])
        values = [step, *[int(x*denominator) for x in distribution], int(survival*denominator), denominator]
        lines.append(' '.join(map(str, values)))
        if step < horizon:
            lifetime += survival
            distribution = [sum(distribution[i]*matrix[i][j] for i in range(3)) for j in range(3)]
    assert 0 <= lifetime <= horizon
    if variant == 1: assert lifetime == 4*(1-F(3,4)**horizon)
    if variant == 2: assert distribution[2] == 0 and lifetime == horizon
    if variant == 3: assert distribution[2] == 1 and lifetime == 0
    if variant == 4: assert lifetime == min(horizon,2)
    if variant == 5: assert lifetime == 2*(1-F(1,2)**horizon)
    lines.append(f'expected_min_lifetime {int(lifetime*denominator)} {denominator}')
    return '\n'.join(lines)+'\n'
def freeze():
    cases = []
    for variant in range(6):
        for horizon in (0,1,2,4,8):
            name = f'{variant}{horizon}'
            (ROOT/'data'/f'{name}.stdin').write_text(name+'\n')
            (ROOT/'data'/f'{name}.stdout').write_text(expected(variant,horizon))
            cases.append({'name':name,'exit':0})
    for name, payload in {'bad_variant':b'68\n','bad_horizon':b'09\n','nondigit':b'x4\n','empty':b'','short':b'0\n','eof':b'04','long':b'040\n','crlf':b'04\r\n'}.items():
        (ROOT/'data'/f'{name}.stdin').write_bytes(payload)
        (ROOT/'data'/f'{name}.stdout').write_text('invalid input: expected variant 0..5 and horizon 0..8 followed by LF\n')
        cases.append({'name':name,'exit':64})
    (ROOT/'data'/'cases.json').write_text(json.dumps({'tolerance':0,'cases':cases},indent=2)+'\n')
if __name__ == '__main__':
    if sys.argv[1:] == ['--freeze']: freeze()
    else:
        for variant in range(6):
            for horizon in (0,1,2,4,8):
                assert (ROOT/'data'/f'{variant}{horizon}.stdout').read_text() == expected(variant,horizon)
        print('30 frozen exact reference cases verified')
