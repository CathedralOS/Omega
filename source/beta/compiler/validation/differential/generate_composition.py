#!/usr/bin/env python3
# Emit one deterministic composed Beta case selected by `SEED`: pre-loop arithmetic, a data-
# dependent linear loop, and POST-loop arithmetic on its result. The straight-line fuzzer tests pure
# arithmetic and the loop fuzzer tests a bare loop returning an accumulator; NEITHER tests the INTERACTION —
# a loop's summarized closed form flowing into further computation (and a computed value feeding the loop's
# delta). That interaction is where the two summarization mechanisms meet, so it is the most valuable thing to
# exercise in the bounded symbolic differential.
#
# Shape: read n + 1-2 data inputs; an optional pre-loop `let` (invariant +/* arithmetic over the inputs — no
# `-`: a ℤ-pair invariant feeding a loop delta is a known conservative refusal); a unit-stride counter loop
# (up `i < n` / `i <= n`, or ~20% down `i > 0` from n) with 1-2 accumulators updated `acc = acc ± δ`; then
# `state done { return <+/-/* arith over the accumulators + inputs + constants> }` — summarized loop results,
# possibly ℤ pairs, flowing through further terms. Small constants keep values under the mod-256 truncation.
import sys, random

def _expr(rng, names, depth, ops=('+', '*')):
    if depth <= 0 or rng.random() < 0.5:
        return rng.choice(names) if (names and rng.random() < 0.7) else str(rng.randint(0, 3))
    return '(%s %s %s)' % (_expr(rng, names, depth - 1, ops), rng.choice(ops), _expr(rng, names, depth - 1, ops))

def _body_delta(rng, invariants):
    # ~35% a COUNTER-LINEAR increment a1·i + a0 (bare i / a·i / a+i / (a·i)+b), degree ≤ 1 in the counter so
    # both sides summarize; the rest a loop-invariant increment. The summarized accumulator (e.g. a·g(n)) then
    # flows into the post-loop arithmetic — exercising the general-linear summarizer COMPOSED with more terms.
    if rng.random() < 0.35:
        coef = None if rng.random() < 0.45 else _expr(rng, invariants, 0)
        term = 'i' if coef is None else '(%s * i)' % coef
        if rng.random() < 0.5:
            term = '(%s + %s)' % (term, _expr(rng, invariants, 0))
        return term
    return _expr(rng, invariants, 1)

def program(seed):
    rng = random.Random(seed)
    data = ['x%d' % i for i in range(rng.randint(1, 2))]
    lines = ['proc main() {', '    let n = read_byte()']
    for x in data:
        lines.append('    let %s = read_byte()' % x)
    invariants = list(data)                                        # values usable in a loop-invariant delta
    if rng.random() < 0.6:                                         # an optional pre-loop computed invariant
        lines.append('    let p = %s' % _expr(rng, data, 2))
        invariants.append('p')
    accs = ['acc%d' % i for i in range(rng.randint(1, 2))]
    for acc in accs:
        lines.append('    let %s = %s' % (acc, rng.choice(['0', str(rng.randint(0, 3))] + data)))
    # ~20% DOWN-counting (i drains n -> 0 under `i > 0`, exercising the >-guard normalization); else up-count.
    down = rng.random() < 0.20
    lines.append('    let i = %s' % ('n' if down else '0'))
    if down:
        lines.append('    state loop { to body when (i > 0)  to done }')
        step = 'i = i - 1'
    else:
        guard = rng.choice(['<', '<='])
        lines.append('    state loop { to body when (i %s n)  to done }' % guard)
        step = 'i = i + 1'
    # ~25% of accumulators SUBTRACT their delta (a ℤ pair flowing into the post-loop arithmetic).
    body = '  '.join('%s = %s %s %s' % (acc, acc, '-' if rng.random() < 0.25 else '+',
                                        _body_delta(rng, invariants)) for acc in accs)
    lines.append('    state body { %s  %s  to loop }' % (body, step))
    # POST-loop arithmetic may also subtract: summarized results (possibly ℤ pairs) flow through +/-/*.
    lines.append('    state done { return %s }' % _expr(rng, accs + data, 2, ops=('+', '-', '*')))
    lines.append('}')
    return '\n'.join(lines) + '\n'

if __name__ == '__main__':
    sys.stdout.write(program(int(sys.argv[1])))
