"""Independent host mathematical model, never a shipped shared implementation.

Fixtures and tolerances were frozen before this program was executed. Exact
Fraction normal-equation witnesses certify the authored solution independently
of this Householder algorithm; production must never solve normal equations.
"""
import argparse
from fractions import Fraction
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def matrix(fixture):
    rows, columns = fixture['rows'], fixture['columns']
    stride = fixture.get('leading_dimension', rows)
    return [[fixture['backing'][row + column * stride]
             for column in range(columns)] for row in range(rows)]


def multiply(left, right):
    return [[math.fsum(a*b for a,b in zip(row, column))
             for column in zip(*right)] for row in left]


def qr_solve(original, rhs, relative=1e-12, absolute=0.0):
    rows, columns = len(original), len(original[0])
    factor = [row[:] for row in original]  # explicit copy
    transformed = rhs[:]
    orthogonal = [[float(i == j) for j in range(rows)] for i in range(rows)]
    scale = max(math.hypot(*column) for column in zip(*original))
    cutoff = absolute + relative * scale
    reflectors = []
    for pivot in range(columns):
        scratch = [factor[row][pivot] for row in range(pivot, rows)]
        norm = math.hypot(*scratch)
        if norm <= cutoff:
            return {'status': 'rank_deficient_unsupported',
                    'first_small_pivot': pivot, 'independent_prefix': pivot}
        alpha = scratch[0]
        beta = -math.copysign(norm, alpha if alpha != 0 else 1.0)
        tau = 1 - alpha / beta
        divisor = alpha / norm - beta / norm
        reflector = [1.0] + [(entry / norm) / divisor for entry in scratch[1:]]
        # Scratch has no aliases into factor, original, or RHS.
        reflectors.append((pivot, reflector[:], tau))
        for column in range(pivot, columns):
            dot = math.fsum(reflector[row-pivot] * factor[row][column]
                            for row in range(pivot, rows))
            for row in range(pivot, rows):
                factor[row][column] -= tau * reflector[row-pivot] * dot
        factor[pivot][pivot] = beta
        for row in range(pivot+1, rows):
            factor[row][pivot] = 0.0
        dot = math.fsum(reflector[row-pivot] * transformed[row]
                        for row in range(pivot, rows))
        for row in range(pivot, rows):
            transformed[row] -= tau * reflector[row-pivot] * dot
    for pivot, reflector, tau in reversed(reflectors):
        for column in range(rows):
            dot = math.fsum(reflector[row-pivot] * orthogonal[row][column]
                            for row in range(pivot, rows))
            for row in range(pivot, rows):
                orthogonal[row][column] -= tau * reflector[row-pivot] * dot
    solution = [0.0] * columns
    for row in reversed(range(columns)):
        tail = math.fsum(factor[row][column] * solution[column]
                         for column in range(row+1, columns))
        solution[row] = (transformed[row] - tail) / factor[row][row]
    residual = [math.fsum(a*x for a,x in zip(row,solution))-b
                for row,b in zip(original,rhs)]
    thin_q = [row[:columns] for row in orthogonal]
    upper = factor[:columns]
    gram = multiply(list(zip(*thin_q)), thin_q)
    reconstructed = multiply(thin_q, upper)
    orthogonality = math.hypot(*(gram[i][j] - float(i == j)
                                for i in range(columns) for j in range(columns)))
    reconstruction = math.hypot(*(original[i][j]-reconstructed[i][j]
                                  for i in range(rows) for j in range(columns)))
    return dict(status='full_rank',solution=solution,residual=residual,
                residual_squared=math.fsum(value*value for value in residual),
                residual_norm2=math.hypot(*residual),
                transformed_tail_norm2=math.hypot(*transformed[columns:]),
                normal_residual=[math.fsum(a*r for a,r in zip(column,residual))
                                 for column in zip(*original)],
                orthogonality_frobenius=orthogonality,
                reconstruction_frobenius=reconstruction,thin_q=thin_q,r=upper)


def qualify(name):
    fixture = json.loads((ROOT/name/'oracle.json').read_text())
    original = matrix(fixture)
    # Exact solution and residual stationarity are checked without QR.
    exact = [[Fraction(value) for value in row] for row in original]
    solution = list(map(Fraction, fixture['expected_solution']))
    residual = [sum(a*x for a,x in zip(row,solution))-Fraction(b)
                for row,b in zip(exact,fixture['right_hand_side'])]
    assert residual == list(map(Fraction,fixture['expected_residual']))
    assert sum(value*value for value in residual) == fixture['expected_residual_squared']
    assert all(sum(a*r for a,r in zip(column,residual)) == 0 for column in zip(*exact))
    # Both fixture Gram determinants are positive (independent uniqueness witness).
    gram = [[sum(row[i]*row[j] for row in exact) for j in range(2)] for i in range(2)]
    assert gram[0][0]*gram[1][1]-gram[0][1]*gram[1][0] > 0
    result = qr_solve(original, fixture['right_hand_side'])
    assert result['status'] == 'full_rank'
    tolerance = fixture['tolerances']
    for actual, expected in zip(result['solution'],fixture['expected_solution']):
        assert abs(actual-expected) <= tolerance['solution_absolute']
    for actual, expected in zip(result['residual'],fixture['expected_residual']):
        assert abs(actual-expected) <= tolerance['residual_absolute']
    assert abs(result['residual_squared']-fixture['expected_residual_squared']) <= tolerance['residual_squared_absolute']
    assert result['orthogonality_frobenius'] <= tolerance['orthogonality_frobenius']
    assert result['reconstruction_frobenius'] <= tolerance['reconstruction_frobenius']
    assert max(map(abs,result['normal_residual'])) <= tolerance['normal_residual_absolute']
    assert abs(result['residual_norm2']-result['transformed_tail_norm2']) <= tolerance['residual_absolute']
    for key in ('thin_q','r'):
        if 'expected_'+key in fixture:
            for actual_row, expected_row in zip(result[key],fixture['expected_'+key]):
                assert all(abs(a-b) <= 1e-12 for a,b in zip(actual_row,expected_row))
    assert original == matrix(fixture)  # original was not mutated
    return result


def rank_table():
    results = []
    for fixture in json.loads((ROOT/'rank_oracles.json').read_text()):
        result = qr_solve(matrix(fixture),fixture['right_hand_side'])
        assert result['status'] == fixture['expected_status'], fixture['name']
        if result['status'] == 'rank_deficient_unsupported':
            assert result['first_small_pivot'] == fixture['first_small_pivot']
        else:
            assert all(abs(a-b) <= 1e-12 for a,b in zip(result['solution'],fixture['expected_solution']))
        results.append(dict(name=fixture['name'],status=result['status'],
                            first_small_pivot=result.get('first_small_pivot')))
    return results


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('project', choices=['tall_line_fit','square_calibration','all'])
    options = parser.parse_args()
    names = ['tall_line_fit','square_calibration'] if options.project == 'all' else [options.project]
    report = {name:qualify(name) for name in names}
    if options.project == 'all':
        report['rank_table'] = rank_table()
    print(json.dumps(report,indent=2,allow_nan=False))
