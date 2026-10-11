#!/usr/bin/env python3
"""Independent stock-by-stock subset partitions; never enumerates Omega codes.
Expected data is frozen before Omega execution. All comparisons are exact integers.
"""
import argparse
import itertools
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parent

def oracle(values):
    fresh, remnant, kerf, *lengths = values
    capacities = [fresh, fresh, remnant]
    subsets = []
    for capacity in capacities:
        subsets.append([tuple(i for i in range(4) if mask & (1 << i))
                        for mask in range(16)
                        if sum(lengths[i] + kerf for i in range(4) if mask & (1 << i)) <= capacity])
    best_score, plans = None, []
    for groups in itertools.product(*subsets):
        used = sum((list(group) for group in groups), [])
        if len(set(used)) != len(used):
            continue
        useful = sum(lengths[i] for i in used)
        opened = sum(capacities[s] for s, group in enumerate(groups) if group)
        score = (len(used), useful, -sum(bool(g) for g in groups[:2]), -opened)
        assignment = [3] * 4
        for source, group in enumerate(groups):
            for item in group:
                assignment[item] = source
        if best_score is None or score > best_score:
            best_score, plans = score, [assignment]
        elif score == best_score:
            plans.append(assignment)
    assignment = min(plans, key=lambda a: sum(s * 4 ** i for i, s in enumerate(a)))
    code = sum(s * 4 ** i for i, s in enumerate(assignment))
    lines = []
    def row(label, *numbers):
        lines.append(label + ' ' + ''.join(f'{n:04d} ' for n in numbers))
    row('summary', best_score[0], best_score[1], -best_score[2], -best_score[3], len(plans), code)
    for i, length in enumerate(lengths):
        row('order', i, length, assignment[i])
    losses, reusable, scrap = 0, 0, 0
    for source, capacity in enumerate(capacities):
        pieces = [lengths[i] for i, s in enumerate(assignment) if s == source]
        loss = len(pieces) * kerf
        leftover = capacity - sum(pieces) - loss if pieces else 0
        row('stock', source, capacity, len(pieces), sum(pieces), loss, leftover)
        losses += loss
        if leftover >= 5:
            reusable += leftover
        else:
            scrap += leftover
    row('certificate', -best_score[3], best_score[1], losses, reusable, scrap)
    assert -best_score[3] == best_score[1] + losses + reusable + scrap
    return '\n'.join(lines) + '\n'

def encode(v):
    return f'{v[0]:03d} {v[1]:03d} {v[2]} ' + ' '.join(f'{n:03d}' for n in v[3:]) + '\n'

def freeze():
    destination = ROOT / 'data' / 'frozen.json'
    if destination.exists():
        raise SystemExit('Refusing to overwrite frozen expectations')
    named = {
        'kerf_ties': [10, 0, 1, 4, 4, 4, 4],
        'remnant_reuse': [10, 6, 1, 5, 4, 3, 2],
        'impossible_lengths': [8, 0, 1, 9, 8, 7, 2],
        'all_impossible': [4, 0, 1, 4, 5, 6, 7],
        'zero_kerf_exact': [8, 0, 0, 4, 4, 4, 4],
        'maximum_lengths': [999, 999, 9, 999, 990, 500, 480],
        'reusable_leftover': [20, 0, 1, 2, 2, 2, 2],
        'eof_without_lf': [10, 0, 1, 4, 4, 4, 4],
    }
    cases = []
    for name, values in named.items():
        text = encode(values)
        if name == 'eof_without_lf':
            text = text.rstrip('\n')
        (ROOT / 'data' / (name + '.stdin')).write_text(text)
        expected = oracle(values)
        (ROOT / 'data' / (name + '.stdout')).write_text(expected)
        cases.append(dict(name=name, input=text, output=expected, exit=0))
    # Feed a reusable leftover from the preceding order into the next remnant slot.
    prior = oracle(named['reusable_leftover'])
    leftover = int(next(line for line in prior.splitlines() if line.startswith('stock 0000')).split()[-1])
    assert leftover == 8
    values = [20, leftover, 1, 3, 3, 1, 1]
    text, expected = encode(values), oracle(values)
    (ROOT / 'data' / 'reuse_round2.stdin').write_text(text)
    (ROOT / 'data' / 'reuse_round2.stdout').write_text(expected)
    cases.append(dict(name='reuse_round2', input=text, output=expected, exit=0))
    for stock, remnant, kerf, lengths in itertools.product(range(3,6), [0,3], [0,1], itertools.product(range(1,4), repeat=4)):
        values = [stock, remnant, kerf, *lengths]
        cases.append(dict(name='tiny_' + '_'.join(map(str, values)), input=encode(values), output=oracle(values), exit=0))
    invalid = ['','010 000 1 004 004 004\n', '010 000 1 004 004 004 004 extra\n',
               '000 000 1 004 004 004 004\n', '010 000 1 000 004 004 004\n',
               '010 000 X 004 004 004 004\n', '010,000 1 004 004 004 004\n',
               '010 000 1 004 004 004 004\r\n', 'x'*40+'\n', '-10 000 1 004 004 004 004\n']
    message = 'INVALID: expected SSS RRR K LLL LLL LLL LLL; stock and orders must be positive\n'
    for i, text in enumerate(invalid):
        (ROOT / 'data' / f'invalid_{i}.stdin').write_text(text)
        cases.append(dict(name=f'invalid_{i}', input=text, output=message, exit=64))
    destination.write_text(json.dumps(dict(model='integer-mm-one-kerf-per-cut', tolerance=0, cases=cases), indent=2)+'\n')
    print(f'Frozen {len(cases)} cases; 972 exhaustive tiny inputs; tolerance 0')

def freeze_reuse():
    destination = ROOT / 'data' / 'frozen_reuse.json'
    if destination.exists():
        raise SystemExit('Refusing to overwrite frozen reuse expectations')
    prior = oracle([20, 0, 1, 2, 2, 2, 2])
    leftover = int(next(line for line in prior.splitlines() if line.startswith('stock 0000')).split()[-1])
    assert leftover == 8
    cases = []
    for name, values in [('remnant_required', [10, 10, 1, 5, 4, 3, 2]),
                         ('reuse_actual_round2', [6, leftover, 1, 3, 3, 1, 1])]:
        text, output = encode(values), oracle(values)
        assert any(int(line.split()[3]) > 0 for line in output.splitlines() if line.startswith('stock 0002'))
        (ROOT / 'data' / (name + '.stdin')).write_text(text)
        (ROOT / 'data' / (name + '.stdout')).write_text(output)
        cases.append(dict(name=name, input=text, output=output, exit=0))
    destination.write_text(json.dumps(dict(model='integer-mm-one-kerf-per-cut', tolerance=0, cases=cases), indent=2)+'\n')
    print('Frozen two additional cases that require remnant cuts')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--freeze', action='store_true')
    group.add_argument('--freeze-reuse', action='store_true')
    arguments = parser.parse_args()
    freeze() if arguments.freeze else freeze_reuse()
