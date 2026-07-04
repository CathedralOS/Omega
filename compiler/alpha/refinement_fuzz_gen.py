#!/usr/bin/env python3
# refinement_fuzz_gen.py SEED — emit ONE random, straight-line, arithmetic Beta program, chosen
# deterministically from SEED. The refinement fuzzer compiles it with bc, derives what the machine code
# computes (alpha_symbolic) and what the source means (beta_symbolic), and proves the two agree for ALL
# inputs (prover -> check.beta). So this generates the PROGRAM SPACE over which bc is certified correct at
# the instruction level, and differentially hardens the two independent symbolic evaluators against each other.
#
# Fragment (kept inside what both evaluators model exactly): 1-3 input bytes via read_byte(), a few let-bound
# intermediate expressions, and a returned expression — all over `+` and `*` (NO `-` / `/` / loops: symbolic
# subtraction/division and data-dependent control are later slices). Small constants keep the Peano terms and
# the differential values bounded.
import sys, random

def _expr(rng, names, depth):
    if depth <= 0 or rng.random() < 0.45:
        # a variable in scope, or a small literal (weighted toward variables so programs actually use inputs)
        if names and rng.random() < 0.7:
            return rng.choice(names)
        return str(rng.randint(0, 4))
    op = rng.choice(['+', '-', '*'])
    return '(%s %s %s)' % (_expr(rng, names, depth - 1), op, _expr(rng, names, depth - 1))

def program(seed):
    rng = random.Random(seed)
    n_inputs = rng.randint(1, 3)
    names = []
    lines = ['proc main() {']
    for i in range(n_inputs):
        lines.append('    let x%d = read_byte()' % i)
        names.append('x%d' % i)
    for j in range(rng.randint(1, 3)):
        lines.append('    let t%d = %s' % (j, _expr(rng, names, rng.randint(1, 2))))
        names.append('t%d' % j)
    # ~30%: route a value through BYTE MEMORY (a store at a high fixed address, later read as an atom).
    # Symbolic stores are modelled untruncated — the mod-256 observable congruence keeps the byte exact.
    if rng.random() < 0.30:
        addr = 5000 + rng.randint(0, 3)
        lines.append('    byte[%d] = %s' % (addr, _expr(rng, names, 1)))
        names.append('byte[%d]' % addr)
    lines.append('    return %s' % _expr(rng, names, rng.randint(1, 2)))
    lines.append('}')
    return '\n'.join(lines) + '\n'

if __name__ == '__main__':
    sys.stdout.write(program(int(sys.argv[1])))
