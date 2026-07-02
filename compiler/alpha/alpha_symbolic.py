#!/usr/bin/env python3
# alpha_symbolic.py — SYMBOLIC execution of the loop-free arithmetic fragment of an Alpha tape. Instead of
# running the 21-op VM on concrete bytes, it runs it over SYMBOLIC Peano terms: each `read` introduces a fresh
# input variable, `imm`/`add`/`mul`/`mov` build up a term, and `write`/`halt` reports the output as a closed-
# form expression over the inputs. So it answers "what function of its inputs does this machine code compute?"
# WITHOUT running it — the first step of instruction-level REFINEMENT (rungs: the Cathedral endgame).
#
# It is UNTRUSTED and CHECKED, like the other *_ref/_symbolic tools:
#   (a) DIFFERENTIALLY — instantiate the derived expression at random inputs and compare to alpha_ref.py's
#       concrete run (soundness of the symbolic engine, pinned exactly like vm-fuzz pins the seeds); and
#   (b) by PROOF — the derived expression is handed to prover.py, which proves it equals the claimed source
#       meaning FOR ALL INPUTS and emits a certificate the trust anchor (check.beta) validates. So a compiler
#       that emits alpha code computing the wrong function is caught by a REJECTED refinement certificate.
#
# Scope (this slice): the register-only, loop-free, non-negative-arithmetic fragment — imm/mov/add/mul/read/
# write/halt. Any other opcode (jumps, memory, sub/div, call/ret) raises Unsupported: the refinement claim is
# only made for programs in this fragment. Peano terms model UNBOUNDED naturals, so the differential check
# must keep values below the 2^64 wrap and the write/halt mod-256 truncation (small inputs) to stay faithful.
# Encoding mirrors alpha_ref.py (the seed-diamonded reference), NOT the loosely-annotated SEMANTICS.md table.
import sys

class Unsupported(Exception):
    pass

# ---- symbolic Peano terms: ('z',) | ('s',t) | ('p',a,b)=plus | ('m',a,b)=times | ('v',i)=input var --------
def nat(k):                            # a concrete constant as s^k z
    t = ('z',)
    for _ in range(k):
        t = ('s', t)
    return t

def render(t):                         # -> check.beta / prover term syntax
    h = t[0]
    if h == 'z':  return 'z'
    if h == 's':  return '(s %s)' % render(t[1])
    if h == 'v':  return '(v %d)' % t[1]
    return '(%s %s %s)' % (h, render(t[1]), render(t[2]))

def evaluate(t, env):                  # concrete integer value under env: {var_index: int}
    h = t[0]
    if h == 'z':  return 0
    if h == 's':  return 1 + evaluate(t[1], env)
    if h == 'v':  return env[t[1]]
    if h == 'p':  return evaluate(t[1], env) + evaluate(t[2], env)
    return evaluate(t[1], env) * evaluate(t[2], env)     # m

def symexec(tape):
    """Symbolically execute a loop-free arithmetic Alpha tape. Returns (output_term, n_inputs).
    n_inputs is how many `read`s occurred (= the arity of the derived function; inputs are (v 0)..(v n-1)
    in read order). Raises Unsupported on any opcode outside the fragment."""
    R = {}                             # register -> symbolic term ; unset registers read as concrete 0
    def reg(i):
        return R.get(i, ('z',))
    pc = 0
    n_inputs = 0
    output = None
    steps = 0
    while True:
        steps += 1
        if steps > 100000:
            raise Unsupported('step budget exceeded (a loop?)')
        op = tape[pc]
        if op == 0x00:                                   # halt d
            if output is None:
                output = reg(tape[pc + 1])
            return output, n_inputs
        elif op == 0x01:                                 # imm d, k
            k = int.from_bytes(tape[pc + 2:pc + 10], 'little')
            R[tape[pc + 1]] = nat(k); pc += 10
        elif op == 0x02:                                 # mov d, s
            R[tape[pc + 1]] = reg(tape[pc + 2]); pc += 3
        elif op == 0x03:                                 # add d, s  -> plus
            d = tape[pc + 1]; R[d] = ('p', reg(d), reg(tape[pc + 2])); pc += 3
        elif op == 0x05:                                 # mul d, s  -> times
            d = tape[pc + 1]; R[d] = ('m', reg(d), reg(tape[pc + 2])); pc += 3
        elif op == 0x11:                                 # read d  -> fresh input var
            R[tape[pc + 1]] = ('v', n_inputs); n_inputs += 1; pc += 2
        elif op == 0x12:                                 # write s  -> the program's output
            if output is None:
                output = reg(tape[pc + 1])
            return output, n_inputs                      # slice 1: single-output programs
        else:
            raise Unsupported('opcode 0x%02x outside the arithmetic fragment' % op)

def main():
    with open(sys.argv[1], 'rb') as f:
        tape = f.read()
    out, n = symexec(tape)
    sys.stdout.write('%d %s\n' % (n, render(out)))       # "<arity> <output-term>"

if __name__ == '__main__':
    main()
