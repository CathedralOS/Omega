#!/usr/bin/env python3
# delta-fuzz-gen.py SEED — emit ONE random, valid, TERMINATING, int-returning Delta program, chosen
# deterministically from SEED. delta-diamond-py.sh runs it through both interp.gamma and delta_ref.py and
# asserts they agree, differential-testing the meaning substrate over ADTs / match / recursion / arithmetic.
#
# Templates (each terminating by construction):
#   arith    — a nested arithmetic/comparison/if expression over constants
#   nat-rec  — structural recursion over a Peano Nat (match Z / (S m), recurse on m)
#   list-rec — structural recursion over a List of ints (match Nil / (Cons h t), recurse on t)
# div/mod by a possibly-zero value is fine: both evaluators trap identically (exit 132).
import sys, random

OPS = ['+', '-', '*', '/', '%', 'eq', 'lt']

def arith(rng, depth, atoms):                          # atoms = int-valued expressions in scope (strings)
    if depth <= 0 or rng.random() < 0.4:
        if atoms and rng.random() < 0.6:
            return rng.choice(atoms)
        # a spread of constants incl. large ones, so `-` yields negatives and signed lt / div/mod / wraparound
        # get exercised (not just small non-negative values)
        return str(rng.choice([0, 1, 2, 3, 5, 7, 10, 100, 200, 1000, 100000]))
    if rng.random() < 0.15:
        return f'(if {arith(rng, depth - 1, atoms)} {arith(rng, depth - 1, atoms)} {arith(rng, depth - 1, atoms)})'
    return f'({rng.choice(OPS)} {arith(rng, depth - 1, atoms)} {arith(rng, depth - 1, atoms)})'

def gen_nat(rng, d):
    return 'Z' if d <= 0 else f'(S {gen_nat(rng, d - 1)})'

def gen_list(rng, n):
    return 'Nil' if n <= 0 else f'(Cons {rng.randint(0, 12)} {gen_list(rng, n - 1)})'

def t_arith(rng):
    return arith(rng, rng.randint(2, 5), [])

def t_nat(rng):
    base = arith(rng, 2, [])
    step = arith(rng, 2, ['(f m)'])                    # only (f m) is int-valued; n and m are Nats
    d = rng.randint(0, 6)
    return f'(def f (n) (match n (Z {base}) ((S m) {step}))) (f {gen_nat(rng, d)})'

def t_list(rng):
    base = arith(rng, 2, [])
    step = arith(rng, 2, ['h', '(g t)'])               # h is an int head; (g t) the recursive result
    n = rng.randint(0, 6)
    return f'(def g (xs) (match xs (Nil {base}) ((Cons h t) {step}))) (g {gen_list(rng, n)})'

def main():
    rng = random.Random(int(sys.argv[1]))
    sys.stdout.write(rng.choice([t_arith, t_nat, t_list])(rng) + '\n')

main()
