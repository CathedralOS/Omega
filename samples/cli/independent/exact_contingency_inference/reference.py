"""Independent six-cell Cartesian-product oracle; never executes Omega.
Freeze mode writes fixtures/oracles. Verify mode never changes expectations.
"""
from fractions import Fraction
from itertools import product
from math import factorial, prod
from pathlib import Path
import json, hashlib, argparse
ROOT = Path(__file__).resolve().parent
HEADER = 'exact_contingency_inference model=fixed_margins_factorial structural_zeros=conditioned\n'

def oracle(record):
    raw = record[:-1] if record.endswith(b'\n') else record
    if len(raw) != 17 or any(c < 48 or c > 57 for c in raw):
        return 64, 'ERROR malformed_record\n', []
    d = [c-48 for c in raw]
    rows, columns, observed, zeros = d[:2], d[2:5], tuple(d[5:11]), d[11:]
    total = sum(rows)
    if min(rows+columns) <= 0 or total > 8 or total != sum(columns) or max(zeros) > 1:
        return 65, 'ERROR invalid_margins_or_mask\n', []
    def valid(t):
        return (sum(t[:3]) == rows[0] and sum(t[3:]) == rows[1] and
                all(t[j]+t[j+3] == columns[j] for j in range(3)) and
                all(not z or x == 0 for z, x in zip(zeros,t)))
    # Intentionally independent of Omega's two-coordinate enumeration.
    tables = [t for t in product(range(total+1), repeat=6) if valid(t)]
    if not tables:
        return 66, 'ERROR impossible_support\n', tables
    if not valid(observed):
        return 67, 'ERROR invalid_observation\n', tables
    def mass(t): return Fraction(1, prod(factorial(x) for x in t))
    def chi(t): return sum((Fraction(total*x-rows[i//3]*columns[i%3])**2 /
                           (total*rows[i//3]*columns[i%3]) for i,x in enumerate(t)), Fraction(0))
    weight = lambda t: int(mass(t)*factorial(total))
    score = chi(observed)
    denominator = total*prod(rows)*prod(columns)
    partition = sum(weight(t) for t in tables)
    fields = dict(feasible=len(tables), partition=partition, observed_weight=weight(observed),
                  pearson_numerator=int(score*denominator), pearson_denominator=denominator,
                  probability_tail_numerator=sum(weight(t) for t in tables if mass(t)<=mass(observed)),
                  association_tail_numerator=sum(weight(t) for t in tables if chi(t)>=score),
                  tail_denominator=partition, association_ties=sum(chi(t)==score for t in tables))
    assert Fraction(fields['pearson_numerator'], denominator) == score
    return 0, HEADER+''.join(f'{k}={v}\n' for k,v in fields.items()), tables

def record(rows, columns, observed, zeros=(0,)*6):
    return ''.join(map(str, (*rows,*columns,*observed,*zeros))).encode()+b'\n'

CASES = {
    'dense_ties': record((3,3),(2,2,2),(1,1,1,1,1,1)),
    'sparse': record((3,3),(2,2,2),(2,1,0,0,1,2)),
    'different_tail_orders': record((2,6),(1,2,5),(0,0,2,1,2,3)),
    'unequal_margins': record((3,5),(3,3,2),(2,0,1,1,3,1)),
    'structural_zero': record((3,3),(2,2,2),(0,1,2,2,1,0),(1,0,0,0,0,0)),
    'singleton': record((1,2),(1,1,1),(1,0,0,0,1,1),(0,1,1,1,0,0)),
    'impossible': record((3,3),(2,2,2),(1,1,1,1,1,1),(1,1,1,0,0,0)),
    'inconsistent_margins': record((3,3),(2,2,1),(1,1,1,1,1,1)),
    'zero_margin': record((0,3),(1,1,1),(0,0,0,1,1,1)),
    'over_bound': record((4,5),(3,3,3),(1,1,2,2,2,1)),
    'bad_mask': record((3,3),(2,2,2),(1,1,1,1,1,1),(2,0,0,0,0,0)),
    'bad_observation': record((3,3),(2,2,2),(1,1,0,1,1,1)),
    'violates_zero': record((3,3),(2,2,2),(1,1,1,1,1,1),(1,0,0,0,0,0)),
    'bad_character': b'3322211111100000x\n',
    'short': b'33222\n',
    'long': b'332221111110000000\n',
    'carriage_return': b'33222111111000000\r\n',
    'empty': b'',
    'exact_eof': record((3,3),(2,2,2),(2,1,0,0,1,2)).rstrip(b'\n'),
}

def freeze():
    manifest = {'model':'conditional factorial measure at fixed positive 2x3 margins, N<=8',
                'numeric_tolerance':0, 'tie_policy':'inclusive exact equality', 'cases':{}}
    for name, data in CASES.items():
        code, stdout, tables = oracle(data)
        (ROOT/'data'/f'{name}.stdin').write_bytes(data)
        (ROOT/'data'/f'{name}.stdout').write_text(stdout)
        (ROOT/'data'/f'{name}.tables.json').write_text(json.dumps(tables)+'\n')
        manifest['cases'][name] = {'exit':code, 'stdin_sha256':hashlib.sha256(data).hexdigest(),
                                  'stdout_sha256':hashlib.sha256(stdout.encode()).hexdigest()}
    manifest['reference_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    (ROOT/'data/oracle.json').write_text(json.dumps(manifest,indent=2)+'\n')

if __name__ == '__main__':
    parser=argparse.ArgumentParser(); parser.add_argument('--freeze', action='store_true')
    options=parser.parse_args()
    if options.freeze: freeze()
    else:
        manifest=json.loads((ROOT/'data/oracle.json').read_text())
        assert manifest['numeric_tolerance']==0
        for name, expected in manifest['cases'].items():
            data=(ROOT/'data'/f'{name}.stdin').read_bytes()
            code, stdout, tables=oracle(data)
            assert code==expected['exit'] and stdout==(ROOT/'data'/f'{name}.stdout').read_text(),name
            assert tables==[tuple(t) for t in json.loads((ROOT/'data'/f'{name}.tables.json').read_text())],name
        print(f"independent oracle verified: {len(manifest['cases'])} cases, tolerance=0")
