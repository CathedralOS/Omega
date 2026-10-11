#!/usr/bin/env python3
"""Independent rational oracle, frozen before any Omega execution.
Quantization uses Python Fraction and round (nearest, ties to even).
Output retains unreduced fractions to make arithmetic observations inspectable.
"""
from fractions import Fraction
from pathlib import Path
import csv, sys
HEADER = "raw,code,reconstructed_num,reconstructed_den,clip,error_num,error_den,bound_num,bound_den"
def oracle(text):
    values = list(map(int, text.split()))
    width, signedness, low, high, rawlow, rawhigh, physical_low, physical_high, rule, count = values[:10]
    assert 2 <= width <= 8 and signedness in (0, 1) and rule == 0
    assert -1000 <= low < high <= 1000 and -1000 <= rawlow < rawhigh <= 1000
    assert -1000 <= physical_low < physical_high <= 1000 and 1 <= count <= 32
    assert len(values) == 10 + count and all(-1000 <= raw <= 1000 for raw in values[10:])
    levels = 2**width - 1
    denominator = rawhigh - rawlow
    lines = [HEADER]
    for raw in values[10:]:
        calibrated = Fraction(physical_low) + Fraction(raw-rawlow, denominator)*(physical_high-physical_low)
        clipped = -1 if calibrated < low else 1 if calibrated > high else 0
        limited = min(Fraction(high), max(Fraction(low), calibrated))
        rank = round((limited-low)*levels/(high-low))
        code = rank - signedness * 2**(width-1)
        reconstruction_num = low*levels + rank*(high-low)
        error_num = reconstruction_num*denominator - int(calibrated*denominator)*levels
        error_den = levels*denominator
        bound_den = 2*error_den
        # Derive overrange independently of reconstruction/error arithmetic.
        bound = Fraction(high-low, 2*levels) + abs(calibrated-limited)
        scaled_bound = bound*bound_den
        assert scaled_bound.denominator == 1
        bound_num = int(scaled_bound)
        assert abs(Fraction(error_num,error_den)) <= Fraction(bound_num,bound_den)
        lines.append(','.join(map(str,(raw,code,reconstruction_num,levels,clipped,error_num,error_den,bound_num,bound_den))))
    return '\n'.join(lines)+'\n'
def verify(fixture, actual):
    expected = fixture.with_suffix('.expected.csv').read_text()
    assert oracle(fixture.read_text()) == expected, 'frozen oracle drift'
    assert actual == expected, 'Omega output differs from frozen rational oracle'
    configuration=list(map(int,fixture.read_text().split()))
    width,signedness=configuration[:2]
    minimum=-(2**(width-1)) if signedness else 0
    maximum=2**(width-1)-1 if signedness else 2**width-1
    rows=list(csv.DictReader(actual.splitlines()))
    for row in rows:
        r={k:int(v) for k,v in row.items()}
        assert minimum <= r['code'] <= maximum
        assert r['clip'] in (-1,0,1)
        assert r['reconstructed_den'] > 0 and r['error_den'] > 0 and r['bound_den'] > 0
        assert abs(Fraction(r['error_num'],r['error_den'])) <= Fraction(r['bound_num'],r['bound_den'])
if __name__ == '__main__':
    fixture=Path(sys.argv[1])
    if len(sys.argv)>2: verify(fixture,Path(sys.argv[2]).read_text())
    else: print(oracle(fixture.read_text()),end='')
