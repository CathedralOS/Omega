#!/usr/bin/env python3
# refinement_fork_gen.py SEED — emit ONE random BRANCHING Beta program, chosen deterministically from SEED.
# This is the fuzz surface for CONDITIONAL TERMS (fork-to-completion): if-diamonds over symbolic guards, with
# all six comparison spellings, ℤ-pair arms, ~30% nested diamonds, and ~25% a summarizable LOOP inside an
# arm (forks and loop summaries composing). The refinement gate proves bc's compiled output ≡ the source
# meaning for every generated program, ∀ inputs.
import sys, random

OPS = ['<', '<=', '>', '>=', '==', '!=']

def _atom(rng, names):
    return rng.choice(names) if (names and rng.random() < 0.7) else str(rng.randint(0, 4))

def _arith(rng, names, depth=1):
    if depth <= 0 or rng.random() < 0.5:
        return _atom(rng, names)
    return '(%s %s %s)' % (_arith(rng, names, depth - 1), rng.choice(['+', '-', '*']), _atom(rng, names))

def _arm(rng, names, label, nxt, allow_loop):
    """One diamond arm: an assignment to r, optionally via a small summarizable loop."""
    if allow_loop and rng.random() < 0.25:
        return ('    state %s { let i = 0  to %sl }\n'
                '    state %sl { to %sb when (i < %s)  to %sx }\n'
                '    state %sb { r = (r + %s)  i = (i + 1)  to %sl }\n'
                '    state %sx { to %s }\n'
                % (label, label, label, label, names[0], label,
                   label, _atom(rng, names), label, label, nxt))
    return '    state %s { r = %s  to %s }\n' % (label, _arith(rng, names, rng.randint(1, 2)), nxt)

def program(seed):
    rng = random.Random(seed)
    names = ['x%d' % i for i in range(rng.randint(2, 3))]
    lines = ['proc main() {']
    for x in names:
        lines.append('    let %s = read_byte()' % x)
    lines.append('    let r = %s' % _atom(rng, names))
    guard = '(%s %s %s)' % (_atom(rng, names), rng.choice(OPS), _atom(rng, names))
    nested = rng.random() < 0.30
    lines.append('    state s0 { to a0 when %s  to b0 }' % guard)
    src = '\n'.join(lines) + '\n'
    if nested:
        g2 = '(%s %s %s)' % (_atom(rng, names), rng.choice(OPS), _atom(rng, names))
        src += _arm(rng, names, 'a0', 'done', allow_loop=True)
        src += '    state b0 { to a1 when %s  to b1 }\n' % g2
        src += _arm(rng, names, 'a1', 'done', allow_loop=False)
        src += _arm(rng, names, 'b1', 'done', allow_loop=False)
    else:
        src += _arm(rng, names, 'a0', 'done', allow_loop=True)
        src += _arm(rng, names, 'b0', 'done', allow_loop=True)
    src += '    state done { return r }\n}\n'
    return src

if __name__ == '__main__':
    sys.stdout.write(program(int(sys.argv[1])))
