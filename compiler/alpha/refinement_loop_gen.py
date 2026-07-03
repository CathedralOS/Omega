#!/usr/bin/env python3
# refinement_loop_gen.py SEED — emit ONE random DATA-DEPENDENT linear-loop Beta program, chosen
# deterministically from SEED. The refinement loop-fuzzer compiles it with bc and proves bc's output ≡ the
# source meaning for ALL inputs — where BOTH the bytecode side (alpha_symbolic) and the source side
# (beta_symbolic) must SUMMARIZE the loop (symbolic trip count) to the same closed form. This hardens the
# intricate loop-summarization recognizers over a program space, the way refinement_fuzz_gen hardens the
# straight-line engines.
#
# Shape (the recognized linear class): a unit-stride counter `i` from 0 with guard `i < n` (n an input), one
# or two accumulators each updated `acc = acc ± <delta linear in i>` per iteration, and a returned
# accumulator or the counter. Deltas are invariant expressions over the DATA inputs and small constants with
# `+`/`*` (never an accumulator or the counter — that would be nonlinear / Σi, outside the class). Small
# constants + few inputs keep values well under the 2^64 wrap and the write/halt mod-256 truncation.
import sys, random

def _inv(rng, data):                   # a loop-INVARIANT atom: a data input or a small constant
    return rng.choice(data) if (data and rng.random() < 0.55) else str(rng.randint(1, 3))

def _delta(rng, data):
    # ~35% of accumulators use a COUNTER-LINEAR increment a1·i + a0 (a1,a0 loop-invariant) — bare `i` (Σi),
    # `a·i` (→ a·g(n)), `a+i` (→ n·a + g(n)), `(a·i)+b`, … all degree-1 in the counter so both sides summarize.
    # The counter must stay degree ≤ 1: `i·i` etc. are genuinely non-linear and are (correctly) refused, so the
    # generator never emits them. The remaining ~65% use a loop-invariant increment.
    if rng.random() < 0.35:
        coef = None if rng.random() < 0.45 else _inv(rng, data)          # a1 (None means 1)
        term = 'i' if coef is None else '(%s * i)' % coef
        if rng.random() < 0.5:
            term = '(%s + %s)' % (term, _inv(rng, data))                 # + a0
        return term
    base = _inv(rng, data)
    if rng.random() < 0.4:
        return '(%s %s %s)' % (base, rng.choice(['+', '*']), _inv(rng, data))
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
    ret = rng.choice(accs + ['i'])                                  # return an accumulator or the counter
    # ~20% DOWN-counting: i drains n -> 0 under (0 < i), stepping by the ℤ pair -1 — exactly n trips. Both
    # modes draw from the full linear-in-i delta space: under a down-counter the i ↦ n-k substitution folds
    # the linear part into the invariant coefficient and flips the triangular part across the ℤ pair.
    down = rng.random() < 0.20
    lines.append('    let i = %s' % ('n' if down else '0'))
    if down:
        guard = rng.choice(['(0 < i)', '(i > 0)', '(i != 0)'])      # equivalent spellings, all normalized
        lines.append('    state loop { to body when %s  return %s }' % (guard, ret))
        step = 'i = i - 1'
        delta = lambda: _delta(rng, data)
    else:
        guard = rng.choice(['i < n', 'i <= n', 'n > i', 'n >= i', 'i != n'])    # all lower to recognized idioms
        lines.append('    state loop { to body when (%s)  return %s }' % (guard, ret))
        step = 'i = i + 1'
        delta = lambda: _delta(rng, data)
    # ~25% of accumulators SUBTRACT their delta (acc = acc - δ): the value goes negative in ℤ and is carried
    # as a difference pair whose pos/neg components summarize independently (observable mod 256 stays exact).
    body = '  '.join('%s = %s %s %s' % (acc, acc, '-' if rng.random() < 0.25 else '+', delta())
                     for acc in accs)
    lines.append('    state body { %s  %s  to loop }' % (body, step))
    lines.append('}')
    return '\n'.join(lines) + '\n'

if __name__ == '__main__':
    sys.stdout.write(program(int(sys.argv[1])))
