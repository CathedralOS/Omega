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
    for acc in accs:
        lines.append('    let t%s = 0' % acc[-1])                   # rewrite temps MUST be declared: bc emits
                                                                    # divergent code for undeclared assignments
    ret = rng.choice(accs + ['i'])                                  # return an accumulator or the counter
    # ~20% DOWN-counting: i drains n -> 0 under (0 < i), stepping by the ℤ pair -1 — exactly n trips. Both
    # modes draw from the full linear-in-i delta space: under a down-counter the i ↦ n-k substitution folds
    # the linear part into the invariant coefficient and flips the triangular part across the ℤ pair.
    down = rng.random() < 0.20
    # up-counters: ~25% start at a symbolic input (trip = bound ∸ start via the monus constructor) —
    # but != guards only summarize from 0 (from a symbolic start the machine diverges when start > bound).
    start = '0' if down or rng.random() >= 0.25 else data[0]
    lines.append('    let i = %s' % ('n' if down else start))
    if down:
        guard = rng.choice(['(0 < i)', '(i > 0)', '(i != 0)'])      # equivalent spellings, all normalized
        lines.append('    state loop { to body when %s  return %s }' % (guard, ret))
        step = 'i = i - 1'
        delta = lambda: _delta(rng, data)
    else:
        guards = ['i < n', 'i <= n', 'n > i', 'n >= i'] + (['i != n'] if start == '0' else [])
        guard = rng.choice(guards)                                  # all lower to recognized idioms
        lines.append('    state loop { to body when (%s)  return %s }' % (guard, ret))
        step = 'i = i + 1'
        delta = lambda: _delta(rng, data)
    # ~12%: the whole body is a CONDITIONAL-DELTA diamond (if guard: acc += δ1 else acc += δ2) — the body
    # fork-merges and the invariant conditional delta summarizes as n·cond(g, δ1, δ2).
    if not down and start == '0' and rng.random() < 0.20 and data:
        g = '(%s %s %d)' % (rng.choice(data), rng.choice(['<', '<=', '==', '!=']), rng.randint(1, 4))
        d1, d2 = _inv(rng, data), _inv(rng, data)
        lines.append('    state body { to armt when %s  to armf }' % g)
        lines.append('    state armt { %s = %s + %s  to step }' % (accs[0], accs[0], d1))
        lines.append('    state armf { %s = %s + %s  to step }' % (accs[0], accs[0], d2))
        lines.append('    state step { %s  to loop }' % step)
        lines.append('}')
        return '\n'.join(lines) + '\n'
    # ~10%: a BUFFER COPY body (byte[base+i] = read_byte()) with element reads post-loop.
    if not down and start == '0' and rng.random() < 0.10:
        base = 6000 + 512 * rng.randint(0, 3)
        lines.append('    state body { byte[(%d + i)] = read_byte()  %s  to loop }' % (base, step))
        lines.append('}')
        return ('\n'.join(lines) + '\n').replace(
            'return %s }' % ret, 'return (byte[%d] + byte[%d]) }' % (base, base + rng.randint(1, 2)))
    # ~25% of accumulators SUBTRACT their delta (acc = acc - δ); ~20% route it through a REWRITE temp
    # (t = δ; acc = acc ± t) — t is overwritten each iteration and dropped post-loop.
    parts = []
    read_used = False                                               # at most ONE read per body: the
    for acc in accs:                                                # summarizers require stride exactly 1
        op = '-' if rng.random() < 0.25 else '+'
        # ~15%: the delta consumes the INPUT STREAM (one read per iteration, adding OR subtracting — a
        # subtracting read puts the Σ on the pair's neg side). Coefficients / added terms stay loop-invariant.
        if not read_used and rng.random() < 0.15:
            read_used = True
            d = rng.choice(['read_byte()',
                            '(read_byte() * %s)' % _inv(rng, data),
                            '(%s * read_byte())' % _inv(rng, data),
                            '(read_byte() + %s)' % _inv(rng, data)])
        else:
            d = delta()
        if rng.random() < 0.20:
            parts.append('t%s = %s  %s = %s %s t%s' % (acc[-1], d, acc, acc, op, acc[-1]))
        else:
            parts.append('%s = %s %s %s' % (acc, acc, op, d))
    body = '  '.join(parts)
    lines.append('    state body { %s  %s  to loop }' % (body, step))
    lines.append('}')
    return '\n'.join(lines) + '\n'

if __name__ == '__main__':
    sys.stdout.write(program(int(sys.argv[1])))
