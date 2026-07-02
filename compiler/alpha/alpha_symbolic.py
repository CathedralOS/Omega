#!/usr/bin/env python3
# alpha_symbolic.py — SYMBOLIC execution of the loop-free fragment of an Alpha tape. Instead of running the
# 21-op VM on concrete bytes, it runs it over a mix of CONCRETE integers and SYMBOLIC Peano terms: each `read`
# introduces a fresh input variable, `imm`/arithmetic build up a value, and `write`/`halt` reports the output
# as a closed-form expression over the inputs. So it answers "what function of its inputs does this machine
# code compute?" WITHOUT running it — the first step of instruction-level REFINEMENT (the Cathedral endgame).
#
# A value is either a Python int (CONCRETE — addresses, stack-pointer arithmetic, literals) or a Peano-term
# tuple (SYMBOLIC — data derived from inputs). This split is what lets it execute REAL bc output: bc's calling
# convention spills locals to a data stack and threads a frame pointer, but in straight-line code every ADDRESS
# is concrete (the stack pointer only moves by constants), so load/store/call/ret resolve to concrete slots
# while the DATA flowing through them stays symbolic. Arithmetic on two concretes stays concrete (mod 2^64);
# any symbolic operand lifts to a `p`/`m` term. Control flow that would branch on a SYMBOLIC value, symbolic
# subtraction/division, or a symbolic address raises Unsupported — those need the ZZ / loop-invariant slices.
#
# UNTRUSTED and CHECKED, like the *_ref tools: alpha_refinement_check.py (a) DIFFERENTIALLY pins the derived
# expression to alpha_ref.py on random inputs and (b) PROVES it equals the claimed source meaning for ALL
# inputs via prover.py + check.beta. Encoding mirrors alpha_ref.py (the seed-diamonded reference).
import sys

MASK = (1 << 64) - 1
INT_MIN = -(1 << 63)
NAT_CAP = 1 << 20                      # refuse to materialize absurdly large concretes as unary Peano terms

class Unsupported(Exception):
    pass

# ---- symbolic Peano terms: ('z',) | ('s',t) | ('p',a,b)=plus | ('m',a,b)=times | ('v',i)=input var --------
def nat(k):                            # a concrete natural as s^k z
    if k < 0 or k > NAT_CAP:
        raise Unsupported('constant %d too large to model as a Peano term' % k)
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

# ---- the dual value model: Python int = concrete, tuple = symbolic Peano term -----------------------
def _term(v):                          # coerce a value to a Peano term (concretes become s^k z)
    return nat(v) if isinstance(v, int) else v

def _s64(x):                           # a concrete 64-bit word as signed
    return x - (1 << 64) if x >= (1 << 63) else x

def _add(a, b):
    if isinstance(a, int) and isinstance(b, int):
        return (a + b) & MASK
    return ('p', _term(a), _term(b))

def _mul(a, b):
    if isinstance(a, int) and isinstance(b, int):
        return (a * b) & MASK
    return ('m', _term(a), _term(b))

def _sub(a, b):
    if isinstance(a, int) and isinstance(b, int):
        return (a - b) & MASK          # address / stack-pointer arithmetic
    raise Unsupported('symbolic subtraction (needs ZZ integers)')

def _concrete(v, why):
    if not isinstance(v, int):
        raise Unsupported(why)
    return v

def symexec(tape):
    """Symbolically execute a loop-free Alpha tape. Returns (output_term, n_inputs), where inputs are
    (v 0)..(v n-1) in `read` order. Raises Unsupported on anything outside the loop-free, concrete-control,
    non-negative-arithmetic fragment (symbolic branch/address/subtraction, div/mod, byte memory, real loops)."""
    R = {}                             # register -> value (int | term) ; unset = concrete 0
    MEM = {}                           # concrete word address -> value ; the data + return-address stacks
    def reg(i):
        return R.get(i, 0)
    sp = 0x04000000                    # the machine's call-stack pointer (return addresses); grows down
    pc = 0
    n_inputs = 0
    steps = 0
    def imm8(at):
        return int.from_bytes(tape[at:at + 8], 'little')
    while True:
        steps += 1
        if steps > 500000:
            raise Unsupported('step budget exceeded (a data-independent loop?)')
        op = tape[pc]
        if op == 0x00:                                   # halt d
            return _term(reg(tape[pc + 1])), n_inputs
        elif op == 0x12:                                 # write s -> the program's output
            return _term(reg(tape[pc + 1])), n_inputs
        elif op == 0x01:                                 # imm d, k
            R[tape[pc + 1]] = imm8(pc + 2); pc += 10
        elif op == 0x02:                                 # mov d, s
            R[tape[pc + 1]] = reg(tape[pc + 2]); pc += 3
        elif op == 0x03:                                 # add d, s
            d = tape[pc + 1]; R[d] = _add(reg(d), reg(tape[pc + 2])); pc += 3
        elif op == 0x04:                                 # sub d, s
            d = tape[pc + 1]; R[d] = _sub(reg(d), reg(tape[pc + 2])); pc += 3
        elif op == 0x05:                                 # mul d, s
            d = tape[pc + 1]; R[d] = _mul(reg(d), reg(tape[pc + 2])); pc += 3
        elif op == 0x06 or op == 0x07:                   # div / mod
            raise Unsupported('div/mod not modelled yet')
        elif op == 0x0A:                                 # load d, s  (word) — concrete address
            a = _concrete(reg(tape[pc + 2]), 'load from symbolic address'); R[tape[pc + 1]] = MEM.get(a, 0); pc += 3
        elif op == 0x0B:                                 # store d, s (word) — concrete address
            a = _concrete(reg(tape[pc + 1]), 'store to symbolic address'); MEM[a] = reg(tape[pc + 2]); pc += 3
        elif op == 0x08 or op == 0x09:                   # loadb / storeb
            raise Unsupported('byte memory not modelled yet')
        elif op == 0x0C:                                 # jmp a
            pc = imm8(pc + 1)
        elif op == 0x0D:                                 # jz c, a  (concrete condition only)
            c = _concrete(reg(tape[pc + 1]), 'branch on symbolic value'); pc = imm8(pc + 2) if c == 0 else pc + 10
        elif op == 0x0E:                                 # jnz c, a
            c = _concrete(reg(tape[pc + 1]), 'branch on symbolic value'); pc = imm8(pc + 2) if c != 0 else pc + 10
        elif op == 0x0F:                                 # jlt a, b, a2  (signed)
            x = _concrete(reg(tape[pc + 1]), 'branch on symbolic value'); y = _concrete(reg(tape[pc + 2]), 'branch on symbolic value')
            pc = imm8(pc + 3) if _s64(x) < _s64(y) else pc + 11
        elif op == 0x10:                                 # jeq a, b, a2
            x = _concrete(reg(tape[pc + 1]), 'branch on symbolic value'); y = _concrete(reg(tape[pc + 2]), 'branch on symbolic value')
            pc = imm8(pc + 3) if x == y else pc + 11
        elif op == 0x11:                                 # read d -> fresh input var
            R[tape[pc + 1]] = ('v', n_inputs); n_inputs += 1; pc += 2
        elif op == 0x13:                                 # call a — push return offset to the call stack
            sp -= 8; MEM[sp] = pc + 9; pc = imm8(pc + 1)
        elif op == 0x14:                                 # ret — pop return offset
            pc = _concrete(MEM.get(sp, 0), 'corrupt return address'); sp += 8
        else:
            raise Unsupported('opcode 0x%02x' % op)

def main():
    with open(sys.argv[1], 'rb') as f:
        tape = f.read()
    out, n = symexec(tape)
    sys.stdout.write('%d %s\n' % (n, render(out)))       # "<arity> <output-term>"

if __name__ == '__main__':
    main()
