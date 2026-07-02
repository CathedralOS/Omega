#!/usr/bin/env python3
# beta-fuzz-gen.py SEED — emit ONE random, valid, terminating Beta program to stdout, chosen
# deterministically from SEED. Used by beta-correctness-fuzz.sh to differential-test the Beta COMPILER:
# the program is both interpreted (beta_interp.py) and compiled (bc) + run (the VM), and the two must
# agree — so a bc miscompile shows up as a disagreement.
#
# Shape: a DAG of value-returning procs (proc f{i} may call only f{j>i}) plus main, over the arithmetic +
# comparison surface with parameters and nested calls. The DAG guarantees termination (no cycles). div/mod
# by a possibly-zero value is fine — interpreter and compiled code trap identically (exit 132). This
# hammers expression codegen (precedence, 64-bit wraparound, signed div/mod, comparisons) and the calling
# convention (args, nesting, depth); control flow / memory / I/O are left to later fuzz slices.
import sys, random

BIN = ['+', '-', '*', '/', '%', '<', '>', '<=', '>=', '==', '!=']

def gen_expr(rng, depth, params, callees):
    if depth <= 0 or rng.random() < 0.35:
        if params and rng.random() < 0.5:
            return rng.choice(params)
        return str(rng.choice([0, 1, 2, 3, 5, 7, 10, 42, 100, 255]))
    r = rng.random()
    if callees and r < 0.3:
        name, arity = rng.choice(callees)
        args = ', '.join(gen_expr(rng, depth - 1, params, callees) for _ in range(arity))
        return f'{name}({args})'
    op = rng.choice(BIN)
    a = gen_expr(rng, depth - 1, params, callees)
    b = gen_expr(rng, depth - 1, params, callees)
    return f'({a} {op} {b})'

def main():
    rng = random.Random(int(sys.argv[1]))
    k = rng.randint(1, 4)                               # helper procs f0..f{k-1}, callable in a DAG
    arities = [rng.randint(1, 3) for _ in range(k)]
    procs = []
    for i in range(k):
        params = ['a', 'b', 'c'][:arities[i]]
        callees = [(f'f{j}', arities[j]) for j in range(i + 1, k)]     # only higher-index procs
        body = gen_expr(rng, rng.randint(2, 4), params, callees)
        procs.append(f'proc f{i}({", ".join(params)}) {{ return {body} }}')
    main_callees = [(f'f{j}', arities[j]) for j in range(k)]
    main_body = gen_expr(rng, rng.randint(2, 4), [], main_callees)
    procs.append(f'proc main() {{ return {main_body} }}')
    sys.stdout.write('\n'.join(procs) + '\n')

main()
