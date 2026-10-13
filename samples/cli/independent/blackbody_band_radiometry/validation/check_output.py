"""Validate frozen independent numerical expectations; never regenerate them."""
import csv
import hashlib
import json
from decimal import Decimal
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent
ORACLE = json.loads((ROOT/'validation/frozen.json').read_text())


def frozen():
    for line in (ROOT/'validation/FROZEN_SHA256.txt').read_text().splitlines():
        digest, path = line.split('  ', 1)
        assert hashlib.sha256((ROOT/path).read_bytes()).hexdigest() == digest, path


def check(name, output, code):
    case = ORACLE['cases'][name]
    assert code == case['exit'], (name, code, output)
    if code:
        assert output == b'ERROR invalid input\n', (name, output)
        return
    rows = list(csv.reader(output.decode().splitlines()))
    assert len(rows) == 2 and rows[0] == ['emitted_W','ambient_W','net_W','emission_fraction'], (name, rows)
    assert len(rows[1]) == 4
    for index, (actual, expected) in enumerate(zip(rows[1], case['expected'])):
        value, reference = Decimal(actual), Decimal(expected)
        assert value.is_finite()
        floor = Decimal('1e-12')
        limit = max(floor, abs(reference)*Decimal('1e-6'))
        assert abs(value-reference) <= limit, (name, index, value, reference, limit)


def relationships(outputs):
    def values(name):
        return list(map(Decimal, outputs[name].decode().splitlines()[1].split(',')))
    room, left, right, swapped = map(values, ['room','left','right','swapped'])
    for index in range(3):
        assert abs(room[index]-left[index]-right[index]) <= max(Decimal('1e-12'),abs(room[index])*Decimal('1e-6'))
    assert abs(room[2]+swapped[2]) <= Decimal('1e-6')*abs(room[2])
    assert values('equal')[2] == 0
    sigma = Decimal(ORACLE['sigma'])
    wide = values('wide')
    assert abs(wide[0]-sigma*Decimal(3000)**4) <= sigma*Decimal(3000)**4*Decimal('1e-6')
    assert abs(wide[3]-1) <= Decimal('1e-6')
