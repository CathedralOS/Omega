#!/usr/bin/env python3
# refinement_nested_gen.py SEED — emit ONE random NESTED-loop Beta program, chosen deterministically from
# SEED. This is the fuzz surface for RECURSIVE loop summarization: an outer data-dependent loop whose body
# contains an inner loop that is itself summarized (concrete bound -> unrolled; symbolic bound -> summarized
# recursively, its closed form over the outer run's markers becoming the outer delta). The refinement gate
# proves bc's compiled output ≡ the source meaning for every generated program, ∀ inputs.
#
# Shape constraints keep every program inside the certifiable class:
#   - inner bound IB ∈ {2, 3, m (an input), i (the outer counter — the TRIANGULAR case)};
#   - inner deltas are a1·j + a0 with a1,a0 loop-invariant; when IB == i the inner delta must be j-INVARIANT
#     (a1 = 0), because an inner g(i) would make the outer delta quadratic (tetrahedral — refused);
#   - the outer step may add its own invariant or i-linear delta after the inner loop;
#   - ~25% of accumulator updates SUBTRACT (ℤ difference pairs flowing through the nest).
import sys, random

def _inv(rng, data):                   # a loop-INVARIANT atom: a data input or a small constant
    return rng.choice(data) if (data and rng.random() < 0.6) else str(rng.randint(1, 3))

def _jdelta(rng, data, allow_jlinear):
    if allow_jlinear and rng.random() < 0.4:                          # a1·j (+ a0), degree 1 in j
        coef = None if rng.random() < 0.5 else _inv(rng, data)
        term = 'j' if coef is None else '(%s * j)' % coef
        if rng.random() < 0.5:
            term = '(%s + %s)' % (term, _inv(rng, data))
        return term
    base = _inv(rng, data)
    if rng.random() < 0.35:
        return '(%s %s %s)' % (base, rng.choice(['+', '*']), _inv(rng, data))
    return base

def program(seed):
    rng = random.Random(seed)
    data = ['a'] + (['b'] if rng.random() < 0.4 else [])
    ib = rng.choice(['2', '3', 'm', 'i'])                             # inner bound: concrete / input / counter
    inputs = ['n'] + (['m'] if ib == 'm' else []) + data
    lines = ['proc main() {']
    for x in inputs:
        lines.append('    let %s = read_byte()' % x)
    lines.append('    let total = %s' % rng.choice(['0', str(rng.randint(0, 3))]))
    lines.append('    let i = 0')
    lines.append('    state outer { to obody when (i < n)  to done }')
    lines.append('    state obody { let j = 0  to inner }')
    lines.append('    state inner { to ibody when (j < %s)  to onext }' % ib)
    op = '-' if rng.random() < 0.25 else '+'
    lines.append('    state ibody { total = (total %s %s)  j = (j + 1)  to inner }'
                 % (op, _jdelta(rng, data, allow_jlinear=(ib != 'i'))))
    extra = ''
    if rng.random() < 0.4:                                            # an outer-step delta after the inner loop
        oop = '-' if rng.random() < 0.25 else '+'
        od = 'i' if rng.random() < 0.3 else _inv(rng, data)
        extra = 'total = (total %s %s)  ' % (oop, od)
    lines.append('    state onext { %si = (i + 1)  to outer }' % extra)
    lines.append('    state done { return total }')
    lines.append('}')
    return '\n'.join(lines) + '\n'

if __name__ == '__main__':
    sys.stdout.write(program(int(sys.argv[1])))
