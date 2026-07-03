#!/usr/bin/env python3
# refinement_compose_gen.py SEED — emit ONE random COMPOSED Beta program: pre-loop arithmetic, a data-
# dependent linear loop, and POST-loop arithmetic on its result. The straight-line fuzzer tests pure
# arithmetic and the loop fuzzer tests a bare loop returning an accumulator; NEITHER tests the INTERACTION —
# a loop's summarized closed form flowing into further computation (and a computed value feeding the loop's
# delta). That interaction is where the two summarization mechanisms meet, so it is the most valuable thing to
# fuzz. The refinement gate proves bc's output ≡ the source meaning for each, ∀ inputs.
#
# Shape: read n + 1-2 data inputs; an optional pre-loop `let` (invariant arithmetic over the inputs); a
# unit-stride counter loop `i < n` / `i <= n` with 1-2 accumulators whose deltas are loop-invariant (over the
# inputs / the pre-loop let / small constants); then `state done { return <arith over the accumulators + inputs
# + constants> }`. Small constants and few inputs keep values under the 2^64 wrap and the mod-256 truncation.
import sys, random

def _expr(rng, names, depth):
    if depth <= 0 or rng.random() < 0.5:
        return rng.choice(names) if (names and rng.random() < 0.7) else str(rng.randint(0, 3))
    return '(%s %s %s)' % (_expr(rng, names, depth - 1), rng.choice(['+', '*']), _expr(rng, names, depth - 1))

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
    lines.append('    let i = 0')
    guard = rng.choice(['<', '<='])
    lines.append('    state loop { to body when (i %s n)  to done }' % guard)
    body = '  '.join('%s = %s + %s' % (acc, acc, _body_delta(rng, invariants)) for acc in accs)
    lines.append('    state body { %s  i = i + 1  to loop }' % body)
    lines.append('    state done { return %s }' % _expr(rng, accs + data, 2))   # POST-loop arithmetic
    lines.append('}')
    return '\n'.join(lines) + '\n'

if __name__ == '__main__':
    sys.stdout.write(program(int(sys.argv[1])))
