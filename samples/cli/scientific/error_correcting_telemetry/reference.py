#!/usr/bin/env python3
"""Independent exact oracle: pair interpolation, not the Omega candidate scan.
Frozen before first Omega execution. No floating point; tolerance is zero.
"""
from itertools import product, combinations
from pathlib import Path
import hashlib, json
ROOT = Path(__file__).resolve().parent

def polynomial(a, b):
    return tuple((a + b*x) % 5 for x in range(4))

def decode(received):
    known = [(x, y) for x, y in enumerate(received) if y is not None]
    candidates = set()
    for (x, y), (z, w) in combinations(known, 2):
        slope = (w-y)*pow(z-x, -1, 5) % 5
        intercept = (y-slope*x) % 5
        code = polynomial(intercept, slope)
        errors = sum(code[i] != value for i, value in known)
        if 2*errors + 4-len(known) <= 2:
            candidates.add((intercept, slope))
    assert len(candidates) <= 1
    return next(iter(candidates), None)

def syndrome(received):
    values = tuple(0 if x is None else x for x in received)
    # Independent parity matrix annihilates every affine polynomial over F5.
    return ''.join(str(sum(a*b for a,b in zip(row, values)) % 5)
                   for row in ((1,3,1,0), (0,1,3,1)))

def response(received):
    found = decode(received)
    suffix = ' S ' + syndrome(received) + '\n'
    if found is None:
        return 'U' + suffix
    code = polynomial(*found)
    mask = ''.join(str(int(x is None or x != y)) for x,y in zip(received, code))
    return f'R {found[0]}{found[1]} L {mask}' + suffix

def freeze():
    data = ROOT/'data'
    commands, expected = [], []
    codebook = {polynomial(a,b): (a,b) for a,b in product(range(5), repeat=2)}
    assert len(codebook) == 25
    assert min(sum(x != y for x,y in zip(a,b)) for a,b in combinations(codebook,2)) == 3
    within = 0
    for code, record in codebook.items():
        assert syndrome(code) == '00'
        for received in product((*range(5), None), repeat=4):
            errors = sum(x is not None and x != y for x,y in zip(received,code))
            erasures = received.count(None)
            if 2*errors+erasures <= 2:
                assert decode(received) == record
                within += 1
    counts = {'recovered':0, 'unrecoverable':0, 'malformed':0}
    for a,b in product(range(5), repeat=2):
        commands.append(f'E{a}{b}\n')
        expected.append('C '+''.join(map(str,polynomial(a,b)))+'\n')
    for received in product((*range(5), None), repeat=4):
        commands.append('D'+''.join('_' if x is None else str(x) for x in received)+'\n')
        output = response(received)
        expected.append(output)
        counts['recovered' if output.startswith('R') else 'unrecoverable'] += 1
    malformed = ['\n','X0123\n','D012\n','D01234\n','D0129\n','E_1\n','E99\n','D0 23\n','D0123\r\n']
    commands.extend(malformed)
    expected.extend(['M\n']*len(malformed))
    counts['malformed'] = len(malformed)
    files = {'exhaustive.stdin': ''.join(commands), 'exhaustive.stdout': ''.join(expected),
             'oversized.stdin': 'D0123000\n', 'oversized.stdout':'LIMIT\n',
             'truncated.stdin':'D0123', 'truncated.stdout':'M\n',
             'empty.stdin':'', 'empty.stdout':'',
             'limit.stdin':'\n'*2000, 'limit.stdout':'M\n'*2000+'LIMIT\n'}
    hashes = {}
    for name, content in files.items():
        payload = content.encode()
        (data/name).write_bytes(payload)
        hashes[name] = hashlib.sha256(payload).hexdigest()
    report = {'model':'RS(4,2), F5, x=0,1,2,3; two synthetic ordinal record fields',
              'tolerance':0, 'minimum_distance':3, 'received_frames':6**4,
              'within_radius_transmissions':within, 'encoder_records':25,
              'counts':counts, 'sha256':hashes}
    (data/'oracle.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report, indent=2))

if __name__ == '__main__':
    freeze()
