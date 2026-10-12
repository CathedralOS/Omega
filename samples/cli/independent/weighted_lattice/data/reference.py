#!/usr/bin/env python3
"""Independent Cartesian-product oracle; no Omega enumeration code reused."""
import itertools, json
from fractions import Fraction
from pathlib import Path
ROOT = Path(__file__).resolve().parent
CASES = [(1,1,1,'o'), (4,1,1,'o'), (4,2,1,'o'), (4,1,2,'r'),
         (4,2,2,'o'), (4,2,2,'r'), (3,3,2,'r'), (8,3,3,'r')]
def reference(n,a,b,t):
    histogram = [0]*(n+1)
    for bits in itertools.product((0,1), repeat=n):
        edges = list(zip(bits, bits[1:]))
        if t == 'r': edges.append((bits[-1],bits[0]))
        k = sum(bits)
        histogram[k] += a**k * b**sum(left*right for left,right in edges)
    z = sum(histogram)
    first = sum(k*h for k,h in enumerate(histogram))
    second = sum(k*k*h for k,h in enumerate(histogram))
    text = ('weighted_lattice v1\n' f'sites={n}\nconfigurations={2**n}\nZ={z}\n'
            f'K numerator={first}\nK2 numerator={second}\nmoment denominator={z}\n'
            'histogram: occupied weighted_count\n' + ''.join(f'{k} {h}\n' for k,h in enumerate(histogram)))
    return dict(configurations=2**n,Z=z,first=first,second=second,histogram=histogram,
                mean=str(Fraction(first,z)), variance=str(Fraction(second,z)-Fraction(first,z)**2)), text
if __name__ == '__main__':
    records = {}
    for case in CASES:
        key = ''.join(map(str,case))
        record,text = reference(*case)
        records[key] = record
        (ROOT/f'{key}.stdin').write_text(key+'\n')
        (ROOT/f'{key}.stdout').write_text(text)
    # Frozen identities: uniform complement symmetry and independent Bernoulli.
    uniform,_ = reference(4,1,1,'o')
    assert uniform['histogram'] == list(reversed(uniform['histogram']))
    assert uniform['first']*2 == 4*uniform['Z']
    assert uniform['second']*4 == 4*5*uniform['Z']
    independent,_ = reference(4,2,1,'o')
    assert independent['Z']==3**4 and Fraction(independent['first'],independent['Z'])==Fraction(8,3)
    (ROOT/'expected.json').write_text(json.dumps(records,indent=2)+'\n')
