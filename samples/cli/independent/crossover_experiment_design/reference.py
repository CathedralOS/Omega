"""Independent labelled Cartesian enumeration; no Omega multiset successor."""
from collections import Counter
from itertools import permutations, product
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
INVALID = 'INVALID: expected T,N,C,F followed by LF; T=2..3 N=1..6 C=0..6 F=0..6\n'
EDGES = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)]
CASES = {
    'two_balanced': '2,4,2,0\n',
    'two_tiny': '2,2,1,0\n',
    'two_cap_infeasible': '2,4,1,0\n',
    'two_forbidden_infeasible': '2,2,2,1\n',
    'three_latin': '3,3,2,0\n',
    'three_pair_balanced': '3,6,2,0\n',
    'three_reverse_forbidden': '3,3,2,1\n',
    'three_cap_infeasible': '3,3,1,0\n',
    'three_period_infeasible': '3,4,6,0\n',
    'zero_cap_infeasible': '2,2,0,0\n',
    'invalid_treatment': '4,4,2,0\n',
    'invalid_subjects': '3,0,2,0\n',
    'invalid_cap': '3,3,9,0\n',
    'invalid_edge': '2,2,2,2\n',
    'invalid_character': '3,x,2,0\n',
    'invalid_delimiter': '3;3,2,0\n',
    'invalid_long': '3,3,2,0 extra\n',
    'invalid_eof': '3,3,2,0',
    'invalid_empty': '',
}


def counts(allocation, treatments):
    period = [Counter(row[p] for row in allocation) for p in range(treatments)]
    carry = Counter((a, b) for row in allocation for a, b in zip(row, row[1:]))
    return [[column[t] for t in range(treatments)] for column in period], [
        [carry[a, b] for b in range(treatments)] for a in range(treatments)]


def accepted(allocation, treatments, cap, forbidden):
    if any(sorted(row) != list(range(treatments)) for row in allocation):
        return False
    periods, carry = counts(allocation, treatments)
    return (all(c * treatments == len(allocation) for row in periods for c in row)
            and all(c <= cap for row in carry for c in row)
            and (not forbidden or carry[EDGES[forbidden-1][0]][EDGES[forbidden-1][1]] == 0))


def oracle(text):
    if (len(text) != 8 or text[7] != '\n' or any(text[p] != ',' for p in (1, 3, 5))
            or any(text[p] not in '0123456789' for p in (0, 2, 4, 6))):
        return {'exit': 64, 'stdout': INVALID}
    treatments, subjects, cap, forbidden = (int(text[p]) for p in (0, 2, 4, 6))
    if (treatments not in (2, 3) or not 1 <= subjects <= 6 or cap > 6 or forbidden > 6
            or (treatments == 2 and forbidden not in (0, 1, 3))):
        return {'exit': 64, 'stdout': INVALID}
    sequences = tuple(permutations(range(treatments)))
    selected = None
    labelled_solutions = 0
    examined = 0
    for allocation in product(sequences, repeat=subjects):
        examined += 1
        if accepted(allocation, treatments, cap, forbidden):
            labelled_solutions += 1
            canonical = tuple(sorted(allocation))
            if selected is None or canonical < selected:
                selected = canonical
    if selected is None:
        return {'exit': 2, 'stdout': 'INFEASIBLE: all subject-sequence multisets exhausted\n',
                'examined_labelled': examined, 'labelled_solutions': 0}
    period, carry = counts(selected, treatments)
    lines = ['FEASIBLE: allocation (treatments A=0 B=1 C=2)']
    lines += [f'subject {s}: ' + ' '.join(map(str, row)) for s, row in enumerate(selected)]
    lines += ['PERIOD COUNTS (rows=period, columns=treatment)']
    lines += [' '.join(map(str, row)) + ' ' for row in period]
    lines += ['CARRYOVER COUNTS (rows=previous, columns=next)']
    lines += [' '.join(map(str, row)) + ' ' for row in carry]
    lines += ['CERTIFIED: subject permutations; exact period balance; carryover cap; forbidden edge']
    return {'exit': 0, 'stdout': '\n'.join(lines) + '\n', 'allocation': selected,
            'period_counts': period, 'carryover_counts': carry,
            'examined_labelled': examined, 'labelled_solutions': labelled_solutions}


def tiny_raw_crosscheck():
    # Enumerate cells including repeated treatments rather than assuming permutations.
    for treatments, subjects in [(2, 2), (2, 3), (3, 2), (3, 3)]:
        for cap in (0, 1, 2):
            raw = 0
            for cells in product(range(treatments), repeat=treatments * subjects):
                rows = tuple(cells[s*treatments:(s+1)*treatments] for s in range(subjects))
                raw += accepted(rows, treatments, cap, 0)
            permutation_count = sum(accepted(rows, treatments, cap, 0)
                                    for rows in product(tuple(permutations(range(treatments))), repeat=subjects))
            assert raw == permutation_count, (treatments, subjects, cap, raw, permutation_count)


if __name__ == '__main__':
    tiny_raw_crosscheck()
    expected = {}
    for name, text in CASES.items():
        (ROOT / 'data' / (name + '.stdin')).write_text(text)
        expected[name] = oracle(text)
    (ROOT / 'data' / 'expected.json').write_text(json.dumps(expected, indent=2) + '\n')
    print('Frozen 19 cases; exhaustive raw-cell tiny crosscheck; integer tolerance = 0')
