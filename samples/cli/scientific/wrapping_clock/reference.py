#!/usr/bin/env python3
"""Independent arbitrary-integer oracle; enumerate admissible elapsed times.

Expected files are frozen in data/*.stdout and *.exit, never rewritten by
validation. This tool calculates a comparison result, not executable delivery.
"""
from pathlib import Path
import re
import sys
MAXIMUM = (1 << 63) - 1

def analyze(text):
    def error(code): return (f'ERROR {code}\n', 2)
    words = re.split('[ \t\r\n]+', text.strip(' \t\r\n')) if text.strip(' \t\r\n') else []
    if any(not re.fullmatch('[0-9]{1,19}', word) or int(word) > MAXIMUM for word in words) or len(words) > 53:
        return error(1)
    values = list(map(int, words))
    if len(values) < 11: return error(1)
    count, modulus, lower, upper, anchor = values[:5]
    if not (2 <= count <= 16 and 2 <= modulus <= 65536 and 1 <= lower <= upper <= 100000): return error(2)
    if len(values) != 5 + 3 * count: return error(1)
    rows = [tuple(values[position:position+3]) for position in range(5, len(values), 3)]
    if any(raw >= modulus or sequence > 100000 or host > 100000 for sequence, raw, host in rows): return error(3)
    if anchor % modulus != rows[0][1]: return error(4)
    ticks = [anchor]
    wraps = dropped = 0
    for before, after in zip(rows, rows[1:]):
        if after[0] <= before[0]: return error(5)
        if after[2] <= before[2]: return error(6)
        # Exhaustive candidates deliberately differ from the Omega ceil formula.
        candidates = [delta for delta in range(lower, upper+1) if (before[1] + delta) % modulus == after[1]]
        if not candidates: return error(7)
        if len(candidates) != 1: return error(8)
        delta = candidates[0]
        next_tick = ticks[-1] + delta
        if next_tick - anchor > 100000: return error(9)
        if next_tick > MAXIMUM: return error(10)
        ticks.append(next_tick)
        wraps += next_tick // modulus - ticks[-2] // modulus
        dropped += after[0] - before[0] - 1
    elapsed = [tick-anchor for tick in ticks]
    host_elapsed = [row[2]-rows[0][2] for row in rows]
    numerator = sum(tick*host for tick,host in zip(elapsed,host_elapsed))
    denominator = sum(tick*tick for tick in elapsed)
    residual = max(abs(host*denominator-tick*numerator) for tick,host in zip(elapsed,host_elapsed))
    lines = [f'row {sequence} raw {raw} timestamp {tick} elapsed {relative} host {host}'
             for (sequence,raw,host),tick,relative in zip(rows,ticks,elapsed)]
    lines += [f'wraps {wraps} dropped {dropped}', f'rate {numerator}/{denominator} host_units/tick',
              f'drift {(numerator-denominator)*1000000}/{denominator} ppm_vs_one',
              f'max_residual {residual}/{denominator} host_units']
    return ('\n'.join(lines)+'\n', 0)

if __name__ == '__main__':
    output, code = analyze(Path(sys.argv[1]).read_text())
    sys.stdout.write(output)
    sys.exit(code)
