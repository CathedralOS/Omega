#!/usr/bin/env python3
"""Independent oracle: enumerate integer start vectors, never task orders.

A feasible serial schedule finishes by sum(durations); positive integer durations
and integer renewable capacities admit an integer optimum. The Cartesian search
therefore includes every possible smaller makespan and lexicographic tie.
This is validation only: the deliverable algorithm lives in domain/solver.omg.
"""
import itertools
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def decode(raw):
    if len(raw) >= 256 and b'\n' not in raw[:256]:
        return None
    line = raw.split(b'\n', 1)[0]
    stripped = line.strip(b' \t\r')
    words = re.split(rb'[ \t\r]+', stripped) if stripped else []
    if not words or len(words) > 19:
        return None
    if any(not word.isdigit() or int(word) > 99 for word in words):
        return None
    values = list(map(int, words))
    if len(values) < 3:
        return None
    count, capacity_a, capacity_b = values[:3]
    if count > 4 or capacity_a > 4 or capacity_b > 4 or len(values) != 3 + 4 * count:
        return None
    tasks = [tuple(values[3 + 4 * i:7 + 4 * i]) for i in range(count)]
    for index, (duration, demand_a, demand_b, mask) in enumerate(tasks):
        if not 1 <= duration <= 5 or demand_a > 4 or demand_b > 4:
            return None
        if mask >= 1 << count or mask & (1 << index):
            return None
    return capacity_a, capacity_b, tasks


def feasible(problem, starts):
    capacity_a, capacity_b, tasks = problem
    ends = tuple(start + task[0] for start, task in zip(starts, tasks))
    for index, task in enumerate(tasks):
        for other in range(len(tasks)):
            if task[3] & (1 << other) and ends[other] > starts[index]:
                return False
    for time in range(max(ends, default=0)):
        active = [i for i in range(len(tasks)) if starts[i] <= time < ends[i]]
        if sum(tasks[i][1] for i in active) > capacity_a:
            return False
        if sum(tasks[i][2] for i in active) > capacity_b:
            return False
    return True


def calculate(raw):
    problem = decode(raw)
    if problem is None:
        return dict(exit=2, stdout='INVALID\n', vectors=0)
    tasks = problem[2]
    horizon = sum(task[0] for task in tasks)
    optimum = None
    inspected = 0
    # Different representation and search space from the Omega implementation.
    for starts in itertools.product(*(range(horizon - task[0] + 1) for task in tasks)):
        inspected += 1
        ends = tuple(start + task[0] for start, task in zip(starts, tasks))
        key = max(ends, default=0), starts
        if optimum is not None and key >= optimum:
            continue
        if feasible(problem, starts):
            optimum = key
    if optimum is None:
        return dict(exit=3, stdout='IMPOSSIBLE\n', vectors=inspected)
    span, starts = optimum
    rows = ''.join(f'TASK {i:02} START {start:02} END {start + tasks[i][0]:02}\n'
                   for i, start in enumerate(starts))
    return dict(exit=0, stdout=rows + f'MAKESPAN {span:02}\n',
                starts=starts, makespan=span, vectors=inspected)


def freeze():
    # This function is an authoring operation; validate.py never rewrites oracles.
    cases = {
        'diamond': '4 3 2 2 1 0 0 3 2 1 1 2 1 1 1 1 1 1 6\n',
        'bottleneck': '4 2 1 2 1 0 0 3 2 1 1 2 1 1 1 1 1 1 6\n',
        'priority_trap': '4 2 0 3 1 0 0 2 1 0 0 4 2 0 1 3 1 0 2\n',
        'independent_resources': '3 1 1 3 1 0 0 2 0 1 0 1 1 1 3\n',
        'reverse_precedence': '3 2 1 2 1 1 2 3 1 0 4 1 1 0 0\n',
        'tie': '3 1 0 2 1 0 0 2 1 0 0 2 1 0 0\n',
        'horizon': '4 1 0 5 1 0 0 5 1 0 0 5 1 0 0 5 1 0 0\n',
        'triple_capacity': '3 2 0 2 1 0 0 2 1 0 0 2 1 0 0\n',
        'zero_resource_precedence': '3 0 0 3 0 0 0 2 0 0 1 2 0 0 1\n',
        'no_resources': '2 0 0 4 0 0 0 3 0 0 0\n',
        'zero_tasks': '0 0 0\n',
        'cycle': '2 1 1 1 1 0 2 1 1 0 1\n',
        'excess_demand': '1 1 0 1 2 0 0\n',
        'zero_capacity': '1 0 0 1 0 1 0\n',
        'self_edge': '1 1 1 1 1 1 1\n',
        'absent_predecessor': '1 1 1 1 1 1 2\n',
        'zero_duration': '1 1 1 0 1 1 0\n',
        'long_duration': '1 1 1 6 1 1 0\n',
        'too_many_tasks': '5 1 1\n',
        'capacity_range': '0 5 0\n',
        'demand_range': '1 4 4 1 5 1 0\n',
        'missing_field': '1 1 1 1 1 1\n',
        'extra_field': '0 0 0 1\n',
        'negative': '1 1 1 -1 1 1 0\n',
        'non_decimal': '0 0 x\n',
        'token_overflow': '100 0 0\n',
        'empty_input': '',
        'full_line': '0' * 256,
        'whitespace_eof': '\t2 1 0 2 1 0 0 1 1 0 0\r',
    }
    # Deterministic small variations include arbitrary ID order, one cycle and
    # zero demand/capacity. Their optima are frozen before any Omega execution.
    for number in range(24):
        count = 2 + number % 2
        tasks = []
        for index in range(count):
            duration = 1 + (number + index * 2) % 3
            demand_a = (number // 2 + index) % 3
            demand_b = (number // 3 + index) % 2
            mask = (1 << ((index + 1) % count)) if number % 4 == 0 else 0
            if number % 4 == 1 and index == count - 1:
                mask = 1
            tasks.extend((duration, demand_a, demand_b, mask))
        cases[f'variant_{number:02}'] = ' '.join(map(str, [count, number % 3,
                                                              number % 2, *tasks])) + '\n'
    frozen = {}
    for name, text in cases.items():
        raw = text.encode('ascii')
        (ROOT / 'data' / f'{name}.input').write_bytes(raw)
        result = calculate(raw)
        (ROOT / 'data' / f'{name}.stdout').write_text(result['stdout'])
        frozen[name] = result
    (ROOT / 'data' / 'expected.json').write_text(json.dumps(frozen, indent=2) + '\n')
    print(f'Frozen {len(frozen)} cases; {sum(c["vectors"] for c in frozen.values())} start vectors; exact integer equality, tolerance=0')


if __name__ == '__main__':
    freeze()
