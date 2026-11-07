"""Independent exact oracle: combinations, permutation determinant, rational elimination.

Run once before sample execution. Validation reads frozen.json and verifies its
hash; it never regenerates expected values from sample/native output.
"""
from fractions import Fraction
from itertools import combinations, permutations
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parent


def determinant(matrix):
    total = 0
    for permutation in permutations(range(len(matrix))):
        inversions = sum(permutation[i] > permutation[j]
                         for i in range(len(matrix)) for j in range(i + 1, len(matrix)))
        product = (-1) ** inversions
        for row, column in enumerate(permutation):
            product *= matrix[row][column]
        total += product
    return total


def inverse(matrix):
    size = len(matrix)
    augmented = [[Fraction(value) for value in row] +
                 [Fraction(i == j) for j in range(size)]
                 for i, row in enumerate(matrix)]
    for column in range(size):
        pivot = next(row for row in range(column, size) if augmented[row][column])
        augmented[column], augmented[pivot] = augmented[pivot], augmented[column]
        scale = augmented[column][column]
        augmented[column] = [value / scale for value in augmented[column]]
        for row in range(size):
            if row != column:
                scale = augmented[row][column]
                augmented[row] = [left - scale * right for left, right in
                                  zip(augmented[row], augmented[column])]
    return [row[size:] for row in augmented]


def solve(mode, candidates, size, predictions):
    dimension = 2 if mode == 'linear' else 3
    best = None
    for indices in combinations(range(len(candidates)), size):
        matrix = [[sum(candidates[index][1] * candidates[index][0] ** (row + column)
                       for index in indices) for column in range(dimension)]
                  for row in range(dimension)]
        value = determinant(matrix)
        # combinations emits input indices lexicographically; strict replacement
        # establishes tie behavior independently of the source's reversed mask key.
        if best is None or value > best['determinant']:
            best = dict(selected=list(indices), information=matrix, determinant=value)
    best['status'] = 'identifiable' if best['determinant'] else 'non-identifiable'
    if best['determinant']:
        covariance = inverse(best['information'])
        variances = [sum(Fraction(setting ** (row + column)) * covariance[row][column]
                         for row in range(dimension) for column in range(dimension))
                     for setting in predictions]
        best['covariance'] = [[str(value) for value in row] for row in covariance]
        best['variances'] = [str(value) for value in variances]
    return best


def freeze():
    cases = []
    def valid(name, mode, candidates, size, predictions=(-10, 0, 10)):
        tokens = [str(len(candidates)), str(size)]
        tokens.extend(str(value) for pair in candidates for value in pair)
        tokens.extend(map(str, predictions))
        payload = mode + ' ' + ' '.join(tokens) + '\n'
        expected = solve(mode, candidates, size, predictions)
        cases.append(dict(name=name, input=payload, exit=0 if expected['determinant'] else 3,
                          expected=expected))
    valid('linear_unequal', 'linear', [(-10, 1), (-3, 10), (0, 2), (7, 3), (10, 1)], 3)
    valid('quadratic_unequal', 'quadratic', [(-10, 2), (-4, 9), (0, 1), (3, 7), (10, 3)], 3)
    valid('linear_tie', 'linear', [(-1, 1), (1, 1), (-1, 1), (1, 1)], 2)
    valid('quadratic_tie', 'quadratic', [(-1, 1), (0, 1), (1, 1), (-1, 1), (0, 1), (1, 1)], 3)
    valid('lex_vs_numeric_mask', 'linear', [(1, 4), (1, 3), (-1, 2), (2, 2), (-2, 1)], 2)
    valid('repeated_settings', 'quadratic', [(-2, 1), (-2, 10), (0, 5), (2, 1), (2, 10)], 4)
    valid('linear_singular', 'linear', [(2, 1), (2, 10), (2, 3)], 2)
    valid('quadratic_singular', 'quadratic', [(-2, 1), (2, 10), (-2, 4), (2, 3)], 3)
    valid('undersized_linear', 'linear', [(-10, 10), (0, 10), (10, 10)], 1)
    valid('undersized_quadratic', 'quadratic', [(-10, 10), (0, 10), (10, 10)], 2)
    valid('minimum_linear', 'linear', [(-10, 10), (0, 10), (10, 10)], 3)
    valid('minimum_quadratic', 'quadratic', [(-10, 10), (0, 10), (10, 10)], 3)
    valid('maximum_linear', 'linear', [(x, 10) for x in (-10,-9,-8,-7,-6,-5,5,6,7,8,9,10)], 6)
    valid('maximum_quadratic', 'quadratic', [(x, 10) for x in (-10,-9,-8,-1,0,1,2,6,7,8,9,10)], 6)
    valid('small_determinant', 'quadratic', [(7,1),(8,1),(9,1)], 3, (7,8,10))
    valid('all_equal_tie', 'quadratic', [(10,10)]*12, 6)
    valid('all_candidates', 'linear', [(-7,1),(-2,4),(0,2),(3,9),(5,1),(10,10)], 6, (-7,3,10))
    valid('asymmetric_predictions', 'quadratic', [(-5,3),(-2,7),(1,2),(4,10),(8,1)], 4, (-8,2,9))
    base=cases[0]['input']
    invalid = {
        'empty':'', 'bad_mode':base.replace('linear','cubic'),
        'bad_mode_suffix':base.replace('linear','linears'),
        'short_mode':'linear', 'negative_count':base.replace('5 3','-5 3',1),
        'count_below': 'linear 2 2 -1 1 1 1 0 0 0\n',
        'count_above':'linear 13 2\n', 'size_zero':base.replace('5 3','5 0',1),
        'size_above':base.replace('5 3','5 7',1),
        'size_exceeds_count':'quadratic 3 4 -1 1 0 1 1 1 0 0 0\n',
        'precision_zero':base.replace('-10 1','-10 0'),
        'precision_negative':base.replace('-10 1','-10 -1'),
        'precision_above':base.replace('-10 1','-10 11'),
        'candidate_low':base.replace('-10 1','-11 1'),
        'candidate_high':base.replace('-10 1','11 1'),
        'prediction_bound':base.rstrip()+' 11\n',
        'prediction_high':base.rsplit('10',1)[0]+'11\n',
        'prediction_low':base.rsplit('10',1)[0]+'-11\n',
        'missing': ' '.join(base.split()[:-1]), 'extra':base+'0\n',
        'bad_token':base.replace('-10 1','x 1'),
        'bare_minus':base.replace('-10 1','- 1'),
        'plus_sign':base.replace('-10 1','+1 1'),
        'huge_integer':base.replace('-10 1','100000000000000000000 1'),
        'too_many_bytes':base+' '*(513-len(base)),
    }
    for name,payload in invalid.items():
        cases.append(dict(name=name,input=payload,exit=2,expected={'invalid':True}))
    cases.append(dict(cases[0],name='tabs_crlf',input=base.replace(' ','\t').replace('\n','\r\n')))
    cases.append(dict(cases[1],name='no_final_newline',input=cases[1]['input'].rstrip()))
    cases.append(dict(cases[0],name='exactly_512_bytes',input=base+' '*(512-len(base))))
    for case in cases:
        (ROOT/'data'/f"{case['name']}.stdin").write_bytes(case['input'].encode())
    encoded=(json.dumps(cases,indent=2)+'\n').encode()
    (ROOT/'data/frozen.json').write_bytes(encoded)
    (ROOT/'data/frozen.sha256').write_text(hashlib.sha256(encoded).hexdigest()+'\n')
    (ROOT/'input.txt').write_text(cases[1]['input'])
    print(f'Frozen {len(cases)} cases; sha256 {hashlib.sha256(encoded).hexdigest()}')

if __name__ == '__main__':
    freeze()
