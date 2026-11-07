#!/usr/bin/env python3
"""Independent pre-execution oracle, exact rational interpolation.
Never read compiler output. Expected files are frozen separately; validation
recomputes and compares these files before running Omega.
"""
from fractions import Fraction
from pathlib import Path
import re
ROOT = Path(__file__).resolve().parent
HEADER = 'row stimulus branch prediction observed residual width (response milliunits)\n'
KNOTS = [(0,0,4000),(20,18000,25000),(50,55000,60000),(100,105000,105000)]
def calibration(x, branch):
    for left, right in zip(KNOTS, KNOTS[1:]):
        if left[0] <= x <= right[0]:
            exact = Fraction(left[branch]) + Fraction(x-left[0], right[0]-left[0]) * (right[branch]-left[branch])
            return exact.numerator // exact.denominator
    raise ValueError(x)
def oracle(payload):
    lines = [HEADER.rstrip('\n')]
    previous = None
    direction = 0
    ambiguous = plateaus = reversals = width_max = rows = 0
    residuals = {1: [], 2: []}
    for record in payload.splitlines(keepends=True):
        # Mirror the public grammar and bounded decimal parser, independently.
        malformed = re.fullmatch(rb'[0-9]+ [0-9]+\n', record) is None or len(record) > 64
        if not malformed:
            for token in record[:-1].split(b' '):
                accumulator = 0
                for digit in token:
                    if accumulator > 20000:
                        malformed = True
                        break
                    accumulator = accumulator*10 + digit-48
        if malformed:
            lines.append('ERROR malformed input: expected stimulus response LF')
            return 64, '\n'.join(lines)+'\n'
        x, observed = map(int, record.split())
        if x > 100 or observed > 200000 or rows == 256:
            lines.append('ERROR range: stimulus 0..100, response 0..200000, at most 256 rows')
            return 65, '\n'.join(lines)+'\n'
        if previous is not None:
            if x == previous:
                plateaus += 1
            else:
                new_direction = 1 if x > previous else 2
                reversals += int(direction != 0 and direction != new_direction)
                direction = new_direction
        previous = x
        rows += 1
        forward, reverse = calibration(x,1), calibration(x,2)
        width = reverse-forward
        width_max = max(width_max,width)
        if direction == 0:
            ambiguous += 1
            lines.append(f'{rows} {x} ambiguous NA {observed} NA {width}')
        else:
            prediction = forward if direction == 1 else reverse
            residual = observed-prediction
            residuals[direction].append(abs(residual))
            branch = 'forward' if direction == 1 else 'reverse'
            lines.append(f'{rows} {x} {branch} {prediction} {observed} {residual} {width}')
    for direction, branch in [(1,'forward'),(2,'reverse')]:
        values=residuals[direction]
        lines.append(f'{branch} count sum_abs max_abs {len(values)} {sum(values)} {max(values,default=0)}')
    lines.append(f'trace rows ambiguous plateaus reversals max_width {rows} {ambiguous} {plateaus} {reversals} {width_max}')
    return 0, '\n'.join(lines)+'\n'
if __name__=='__main__':
    import sys
    code, output=oracle(Path(sys.argv[1]).read_bytes())
    print(output,end='')
    raise SystemExit(code)
