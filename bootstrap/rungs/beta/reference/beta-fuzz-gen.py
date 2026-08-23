#!/usr/bin/env python3
# beta-fuzz-gen.py SEED — emit ONE random, valid, TERMINATING Beta program to stdout, chosen
# deterministically from SEED. Used by beta-correctness-fuzz.sh to differential-test the Beta COMPILER:
# the program is both interpreted (beta_interp.py) and compiled (bc) + run (the VM), and the two must
# agree — so a bc miscompile shows up as a disagreement.
#
# The program is one of several TEMPLATES, each terminating by construction, together covering the
# compiler's back end broadly:
#   expr  — a DAG of value-returning procs (proc f{i} calls only f{j>i}): expression codegen + the calling
#           convention + div/mod (incl. traps, which both sides hit identically -> exit 132)
#   loop  — a bounded counting loop accumulating a value: state/CFG control flow + guarded transitions
#   rec   — structural recursion on a decreasing argument: recursion + frames + a base-case guard
#   mem   — fill a word[] buffer then sum it back: byte[]/word[] memory codegen
# NOTE: Beta uses `;` for COMMENTS, so statements are whitespace-separated (never `;`-separated).
import sys, random

def gen_arith(rng, depth, params):                     # +,-,* only (trap-free, value-varied) over params/consts
    if depth <= 0 or rng.random() < 0.4:
        if params and rng.random() < 0.5:
            return rng.choice(params)
        return str(rng.choice([0, 1, 2, 3, 5, 7, 11]))
    op = rng.choice(['+', '-', '*'])
    return f'({gen_arith(rng, depth - 1, params)} {op} {gen_arith(rng, depth - 1, params)})'

def gen_expr(rng, depth, params, callees):             # full surface incl. div/mod + calls (for the DAG)
    if depth <= 0 or rng.random() < 0.35:
        if params and rng.random() < 0.5:
            return rng.choice(params)
        return str(rng.choice([0, 1, 2, 3, 5, 7, 10, 42, 100, 255]))
    r = rng.random()
    if callees and r < 0.3:
        name, arity = rng.choice(callees)
        return f'{name}({", ".join(gen_expr(rng, depth - 1, params, callees) for _ in range(arity))})'
    op = rng.choice(['+', '-', '*', '/', '%', '<', '>', '<=', '>=', '==', '!='])
    return f'({gen_expr(rng, depth - 1, params, callees)} {op} {gen_expr(rng, depth - 1, params, callees)})'

def t_expr(rng):
    k = rng.randint(1, 4)
    arities = [rng.randint(1, 3) for _ in range(k)]
    out = []
    for i in range(k):
        params = ['a', 'b', 'c'][:arities[i]]
        callees = [(f'f{j}', arities[j]) for j in range(i + 1, k)]
        out.append(f'proc f{i}({", ".join(params)}) {{ return {gen_expr(rng, rng.randint(2, 4), params, callees)} }}')
    callees = [(f'f{j}', arities[j]) for j in range(k)]
    out.append(f'proc main() {{ return {gen_expr(rng, rng.randint(2, 4), [], callees)} }}')
    return '\n'.join(out)

def t_loop(rng):
    n = rng.randint(1, 12)
    c0 = rng.randint(0, 9)
    body = gen_arith(rng, rng.randint(1, 3), ['i'])
    op = rng.choice(['+', '-', '*'])
    return (f'proc main() {{\n'
            f'    let i = 0\n'
            f'    let acc = {c0}\n'
            f'    state lp {{ to body when (i < {n})  return acc }}\n'
            f'    state body {{ acc = (acc {op} {body})  i = i + 1  to lp }}\n'
            f'}}')

def t_rec(rng):
    n = rng.randint(1, 8)
    base = rng.randint(0, 9)
    step = gen_arith(rng, rng.randint(1, 3), ['n'])
    return (f'proc rec(n) {{\n'
            f'    state b {{ to r when (n > 0)  return {base} }}\n'
            f'    state r {{ return ({step} + rec(n - 1)) }}\n'
            f'}}\n'
            f'proc main() {{ return rec({n}) }}')

def t_mem(rng):
    n = rng.randint(1, 8)
    cell = gen_arith(rng, rng.randint(1, 3), ['i'])
    return (f'proc main() {{\n'
            f'    let buf = 2097152\n'
            f'    let i = 0\n'
            f'    state fill {{ to sum0 when (i >= {n})  to body }}\n'
            f'    state body {{ word[buf + i * 8] = {cell}  i = i + 1  to fill }}\n'
            f'    state sum0 {{ let s = 0  let j = 0  to loop }}\n'
            f'    state loop {{ to done when (j >= {n})  to acc }}\n'
            f'    state acc {{ s = (s + word[buf + j * 8])  j = j + 1  to loop }}\n'
            f'    state done {{ return s }}\n'
            f'}}')

def main():
    rng = random.Random(int(sys.argv[1]))
    tmpl = rng.choice([t_expr, t_loop, t_rec, t_mem])
    sys.stdout.write(tmpl(rng) + '\n')

main()
