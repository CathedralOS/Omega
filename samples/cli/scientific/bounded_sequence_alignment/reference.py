#!/usr/bin/env python3
"""Independent exhaustive path enumeration and output reconstruction.

Enumeration has no score matrix or traceback recurrence. The deterministic
reference separately implements the declared tie policy, and every emitted path
is reconstructed against the original input and rescored without its matrix.
"""
import itertools
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent
POLICY = json.loads((ROOT / 'data/oracle.json').read_text())

def substitution(a, b):
    return -1 if 'N' in (a, b) else (3 if a == b else -2)

def exhaustive(a, b):
    maximum = 0
    def visit(i, j, score):
        nonlocal maximum
        maximum = max(maximum, score)
        if i < len(a) and j < len(b):
            visit(i + 1, j + 1, score + substitution(a[i], b[j]))
        if i < len(a):
            visit(i + 1, j, score - 2)
        if j < len(b):
            visit(i, j + 1, score - 2)
    for i in range(len(a)):
        for j in range(len(b)):
            visit(i, j, 0)
    return maximum

def deterministic(a, b):
    a, b = a.upper(), b.upper()
    scores = [[0] * (len(b) + 1) for _ in range(len(a) + 1)]
    directions = {}
    maximum, end = 0, (0, 0)
    for i in range(1, len(a) + 1):
        for j in range(1, len(b) + 1):
            candidates = (0, scores[i-1][j-1] + substitution(a[i-1], b[j-1]),
                          scores[i-1][j] - 2, scores[i][j-1] - 2)
            scores[i][j] = max(candidates)
            directions[i, j] = candidates.index(scores[i][j])
            if scores[i][j] > maximum:
                maximum, end = scores[i][j], (i, j)
    i, j = end
    first, second = [], []
    while scores[i][j] > 0:
        direction = directions[i, j]
        if direction == 1:
            i -= 1; j -= 1; first.append(a[i]); second.append(b[j])
        elif direction == 2:
            i -= 1; first.append(a[i]); second.append('-')
        elif direction == 3:
            j -= 1; first.append('-'); second.append(b[j])
        else:
            raise AssertionError('positive cell has no predecessor')
    return (f'score={maximum}\nfirst={i}:{end[0]}\nsecond={j}:{end[1]}\n'
            f'alignment-first={"".join(reversed(first))}\n'
            f'alignment-second={"".join(reversed(second))}\n')

def reconstruct(a, b, text):
    rows = dict(line.split('=', 1) for line in text.splitlines())
    first_start, first_end = map(int, rows['first'].split(':'))
    second_start, second_end = map(int, rows['second'].split(':'))
    first, second = rows['alignment-first'], rows['alignment-second']
    assert len(first) == len(second)
    assert first.replace('-', '') == a.upper()[first_start:first_end]
    assert second.replace('-', '') == b.upper()[second_start:second_end]
    assert 0 <= first_start <= first_end <= len(a)
    assert 0 <= second_start <= second_end <= len(b)
    total = 0
    for left, right in zip(first, second):
        assert (left, right) != ('-', '-')
        total += -2 if '-' in (left, right) else substitution(left, right)
        assert total > 0, 'local path contains a nonpositive prefix'
    assert total == int(rows['score'])
    return total

def short_sequences(maximum):
    return [''.join(word) for length in range(1, maximum + 1)
            for word in itertools.product('ACN', repeat=length)]

def freeze():
    rows = []
    for a, b in POLICY['variants']:
        expected = deterministic(a, b)
        reconstruct(a, b, expected)
        rows.append({'first': a, 'second': b, 'stdout': expected, 'exit': 0})
    (ROOT / 'data/expected.json').write_text(json.dumps(rows, indent=2) + '\n')
    short_rows = [{'first': a, 'second': b, 'stdout': deterministic(a, b), 'exit': 0}
                  for a, b in itertools.product(short_sequences(2), repeat=2)]
    (ROOT / 'data/short_expected.json').write_text(json.dumps(short_rows, indent=2) + '\n')
    assert deterministic('ACGT', 'ACT') == (ROOT / 'data/gap.stdout').read_text()
    pairs = 0
    for a, b in itertools.product(short_sequences(3), repeat=2):
        score = reconstruct(a, b, deterministic(a, b))
        assert score == exhaustive(a, b), (a, b, score)
        pairs += 1
    print(f'frozen variants={len(rows)}; exhaustive reference pairs={pairs}; tolerance=0')

def validate(binary, destination):
    destination.mkdir(parents=True, exist_ok=True)
    rows = json.loads((ROOT / 'data/expected.json').read_text())
    rows.extend(json.loads((ROOT / 'data/short_expected.json').read_text()))
    observations = []
    for index, case in enumerate(rows):
        a, b = case['first'], case['second']
        invocation = subprocess.run([binary], input=f'{a}\n{b}\n'.encode(), capture_output=True, timeout=10)
        output = invocation.stdout.decode('ascii')
        assert invocation.returncode == case['exit'], (a, b, invocation.returncode, invocation.stderr)
        assert output == case['stdout'], (a, b, output, case['stdout'])
        assert not invocation.stderr, invocation.stderr
        score = reconstruct(a, b, output)
        if len(a) <= 3 and len(b) <= 3:
            assert score == exhaustive(a.upper(), b.upper())
        observations.append({'first': a, 'second': b, 'stdout': output, 'exit': invocation.returncode})
    invalid = json.loads((ROOT / 'data/invalid_expected.json').read_text())
    for case in invalid:
        payload = bytes.fromhex(case['input_hex'])
        invocation = subprocess.run([binary], input=payload, capture_output=True, timeout=10)
        assert invocation.returncode == case['exit'], (payload, invocation.returncode)
        assert invocation.stdout == case['stdout'].encode()
        assert not invocation.stderr
        observations.append({'input_hex': payload.hex(), 'exit': invocation.returncode, 'stdout': invocation.stdout.decode()})
    # The second sequence can end at EOF without LF.
    invocation = subprocess.run([binary], input=b'ACGT\nACT', capture_output=True, timeout=10)
    assert invocation.returncode == 0 and invocation.stdout.decode() == deterministic('ACGT', 'ACT')
    observations.append({'input_hex': b'ACGT\nACT'.hex(), 'exit': 0, 'stdout': invocation.stdout.decode()})
    (destination / 'observations.json').write_text(json.dumps(observations, indent=2) + '\n')
    print(f'native exact-output cases={len(rows)}; malformed cases={len(invalid)}; EOF variant=1; traceback reconstruction complete')

if __name__ == '__main__':
    if sys.argv[1:] == ['--freeze']:
        freeze()
    elif len(sys.argv) == 4 and sys.argv[1] == '--validate':
        validate(str(pathlib.Path(sys.argv[2]).resolve()), pathlib.Path(sys.argv[3]))
    else:
        raise SystemExit('usage: reference.py --freeze | --validate BINARY OUTPUT_DIRECTORY')
