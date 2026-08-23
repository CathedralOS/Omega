#!/usr/bin/env python3
# io-fuzz-gen.py SEED — emit ONE random Beta program that READS a single input byte and produces
# input-dependent output + exit, chosen deterministically from SEED. beta-io-exhaust.sh verifies each
# program EXHAUSTIVELY over all 256 possible input bytes (interpret vs compile+run), so a miscompiled
# read_byte / write_byte / input-dependent branch is caught completely, not just sampled.
import sys, random

def arith(rng, depth, params):                         # +,-,* over params + small consts (trap-free)
    if depth <= 0 or rng.random() < 0.4:
        return rng.choice(params) if params and rng.random() < 0.6 else str(rng.choice([1, 2, 3, 5, 7]))
    op = rng.choice(['+', '-', '*'])
    return f'({arith(rng, depth - 1, params)} {op} {arith(rng, depth - 1, params)})'

def t_straight(rng):
    return (f'proc main() {{\n'
            f'    let c = read_byte()\n'
            f'    let x = {arith(rng, 3, ["c"])}\n'
            f'    write_byte({arith(rng, 2, ["c", "x"])})\n'
            f'    return {arith(rng, 3, ["c", "x"])}\n'
            f'}}')

def t_branch(rng):                                     # input-dependent control flow (a comparison boundary)
    thr = rng.randint(0, 255)
    op = rng.choice(['<', '>', '<=', '>=', '==', '!='])
    return (f'proc main() {{\n'
            f'    let c = read_byte()\n'
            f'    state s {{ to hi when (c {op} {thr})  return {arith(rng, 2, ["c"])} }}\n'
            f'    state hi {{ write_byte({arith(rng, 2, ["c"])})  return {arith(rng, 2, ["c"])} }}\n'
            f'}}')

def main():
    rng = random.Random(int(sys.argv[1]))
    sys.stdout.write(rng.choice([t_straight, t_branch])(rng) + '\n')

main()
