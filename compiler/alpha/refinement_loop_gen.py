#!/usr/bin/env python3
# refinement_loop_gen.py SEED — emit ONE random DATA-DEPENDENT linear-loop Beta program, chosen
# deterministically from SEED. The refinement loop-fuzzer compiles it with bc and proves bc's output ≡ the
# source meaning for ALL inputs — where BOTH the bytecode side (alpha_symbolic) and the source side
# (beta_symbolic) must SUMMARIZE the loop (symbolic trip count) to the same closed form. This hardens the
# intricate loop-summarization recognizers over a program space, the way refinement_fuzz_gen hardens the
# straight-line engines.
#
# Shape (the recognized linear class): a unit-stride counter `i` from 0 with guard `i < n` (n an input), one
# or two accumulators each updated `acc = acc + <loop-invariant delta>` per iteration, and a returned
# accumulator or the counter. Deltas are invariant expressions over the DATA inputs and small constants with
# `+`/`*` (never an accumulator or the counter — that would be nonlinear / Σi, outside the class). Small
# constants + few inputs keep values well under the 2^64 wrap and the write/halt mod-256 truncation.
import sys, random

def _delta(rng, data):
    base = rng.choice(data) if (data and rng.random() < 0.55) else str(rng.randint(1, 3))
    if rng.random() < 0.4:
        other = rng.choice(data + [str(rng.randint(1, 3))]) if data else str(rng.randint(1, 3))
        return '(%s %s %s)' % (base, rng.choice(['+', '*']), other)
    return base

def program(seed):
    rng = random.Random(seed)
    data = ['x%d' % i for i in range(rng.randint(1, 2))]
    accs = ['acc%d' % i for i in range(rng.randint(1, 2))]
    lines = ['proc main() {', '    let n = read_byte()']
    for x in data:
        lines.append('    let %s = read_byte()' % x)
    for acc in accs:
        init = rng.choice(['0', str(rng.randint(0, 3))] + data)     # accumulator start: 0, a const, or an input
        lines.append('    let %s = %s' % (acc, init))
    lines.append('    let i = 0')
    ret = rng.choice(accs + ['i'])                                  # return an accumulator or the counter
    guard = rng.choice(['<', '<='])                                 # both lower to a recognized compare idiom
    lines.append('    state loop { to body when (i %s n)  return %s }' % (guard, ret))
    body = '  '.join('%s = %s + %s' % (acc, acc, _delta(rng, data)) for acc in accs)
    lines.append('    state body { %s  i = i + 1  to loop }' % body)
    lines.append('}')
    return '\n'.join(lines) + '\n'

if __name__ == '__main__':
    sys.stdout.write(program(int(sys.argv[1])))
