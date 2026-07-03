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

# TRI_ID: the triangular-sum recurrence g(0)=0, g(s k)=g(k)+k (so g(n)=Σ_{j<n} j). A loop `acc += i` over trip
# count t computes g(t); it stays a checker-accepted closed FORM as ('f', TRI_ID, t) (refl on a symbolic input).
TRI_ID = 90

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
    if h == 'f':  return '(f %d %s)' % (t[1], render(t[2]))
    return '(%s %s %s)' % (h, render(t[1]), render(t[2]))

def evaluate(t, env):                  # concrete integer value under env: {var_index: int}
    h = t[0]
    if h == 'z':  return 0
    if h == 's':  return 1 + evaluate(t[1], env)
    if h == 'v':  return env[t[1]]
    if h == 'f':                       # a user-function recurrence; TRI_ID is the triangular sum g(n)=Σ_{j<n} j
        if t[1] != TRI_ID:
            raise Unsupported('unknown recurrence fun %d' % t[1])
        a = evaluate(t[2], env)
        return a * (a - 1) // 2
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

# ---- data-dependent loop summarization (bytecode side) -----------------------------------------------
# A value ('cmp', 'lt'|'eq', L, R) is a SYMBOLIC boolean produced when a comparison has a symbolic operand
# (bc lowers `i < n` to jlt + an imm 0/1 dance feeding a jz). When such a boolean reaches a jz/jnz that guards
# a loop, we summarize the loop instead of unrolling — the source-side mirror of beta_symbolic._summarize_loop.
_ILEN = {0x00:2,0x01:10,0x02:3,0x03:3,0x04:3,0x05:3,0x06:3,0x07:3,0x08:3,0x09:3,0x0A:3,0x0B:3,
         0x0C:9,0x0D:10,0x0E:10,0x0F:11,0x10:11,0x11:2,0x12:2,0x13:9,0x14:1}

def _le8(tape, a):
    return int.from_bytes(tape[a:a + 8], 'little')

def _back_edges(tape):
    """{header_pc: jmp_pc} for every direct `jmp @t` with t < its own address (a loop's back-edge)."""
    out = {}; pc = 0
    while pc < len(tape):
        op = tape[pc]
        if op not in _ILEN:
            break
        if op == 0x0C:
            t = _le8(tape, pc + 1)
            if t < pc:
                out[t] = pc
        pc += _ILEN[op]
    return out

def _cmp_idiom(tape, pc):
    """Recognize bc's compare-to-boolean lowering at a jlt/jeq:
        jlt rX,rY,Ltrue ; imm rZ,fv ; jmp Lj ; Ltrue: imm rZ,(1-fv) ; Lj:
    so rZ becomes [rX<rY] (standard, fv=0) or ![rX<rY] (swapped, fv=1) — bc emits the swapped polarity with
    operands reversed for `i<=n` (jlt n,i). Returns (rZ, Lj, op, fv) or None."""
    op = tape[pc]
    if op not in (0x0F, 0x10):
        return None
    ltrue = _le8(tape, pc + 3)
    fall = pc + 11
    if tape[fall] != 0x01:
        return None
    rz = tape[fall + 1]
    fv = _le8(tape, fall + 2)
    if fv not in (0, 1):
        return None
    jpc = fall + 10
    if tape[jpc] != 0x0C:
        return None
    lj = _le8(tape, jpc + 1)
    if tape[ltrue] != 0x01 or tape[ltrue + 1] != rz or _le8(tape, ltrue + 2) != (1 - fv) or ltrue + 10 != lj:
        return None
    return (rz, lj, op, fv)

def _run_body_once(tape, start_pc, header, MEM, R, sp):
    """Speculatively execute ONE loop iteration (concrete control only) from start_pc until pc jumps back to
    `header`; return the resulting memory. Operates on copies, so the caller's state is untouched. This is the
    EXACT per-iteration transition — one iteration is straight-line, so no sampling is involved."""
    MEM = dict(MEM); R = dict(R); pc = start_pc; steps = 0
    def reg(i):
        return R.get(i, 0)
    while True:
        steps += 1
        if steps > 20000:
            raise Unsupported('loop body too long to summarize')
        op = tape[pc]
        if op == 0x0C:
            t = _le8(tape, pc + 1)
            if t == header:
                return MEM
            pc = t; continue
        if op == 0x01:   R[tape[pc + 1]] = _le8(tape, pc + 2); pc += 10
        elif op == 0x02: R[tape[pc + 1]] = reg(tape[pc + 2]); pc += 3
        elif op == 0x03: d = tape[pc + 1]; R[d] = _add(reg(d), reg(tape[pc + 2])); pc += 3
        elif op == 0x04: d = tape[pc + 1]; R[d] = _sub(reg(d), reg(tape[pc + 2])); pc += 3
        elif op == 0x05: d = tape[pc + 1]; R[d] = _mul(reg(d), reg(tape[pc + 2])); pc += 3
        elif op == 0x0A: a = _concrete(reg(tape[pc + 2]), 'load'); R[tape[pc + 1]] = MEM.get(a, 0); pc += 3
        elif op == 0x0B: a = _concrete(reg(tape[pc + 1]), 'store'); MEM[a] = reg(tape[pc + 2]); pc += 3
        else:
            raise Unsupported('loop body opcode 0x%02x (only straight-line +/*/mem summarizable)' % op)

def _concnat(d):                       # the int value of a concrete delta (int OR a Peano nat s^k z), else None
    if isinstance(d, int):
        return d
    if d == ('z',):
        return 0
    if isinstance(d, tuple) and d[0] == 's':
        inner = _concnat(d[1])
        return None if inner is None else inner + 1
    return None

def _slot_delta(s0, s1):
    """per-iteration increment given a slot's entry value s0 and its value s1 after one body run (s1 = s0 + d)."""
    if isinstance(s0, int) and isinstance(s1, int):
        return (s1 - s0) & MASK
    if s1 == s0:
        return 0
    if isinstance(s1, tuple) and s1[0] == 'p':          # s1 = (p s0 d) or (p d s0)  ->  d
        if s1[1] == _term(s0):
            return s1[2]
        if s1[2] == _term(s0):
            return s1[1]
    return None

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
    backedges = _back_edges(tape)
    def imm8(at):
        return int.from_bytes(tape[at:at + 8], 'little')

    def summarize(cond, cont_pc, exit_pc):
        """cond = ('cmp','lt'|'eq', L, R) guards a loop that continues to cont_pc and exits to exit_pc. Read the
        per-iteration transition off ONE speculative body run, recognize a unit-stride counter `L < R` from 0
        and loop-invariant accumulator deltas, and replace the loop by each accumulator's closed form
        init + trip*delta (trip = R). Mutates MEM to the post-loop state; returns exit_pc, or None if the loop
        is outside the summarizable class (caller then bails)."""
        kind, L, Rb = cond[1], cond[2], cond[3]
        if kind not in ('lt', 'le') or not isinstance(L, int):
            return None                                 # `counter < bound` / `counter <= bound`, counter concrete at entry
        header = next((h for h, j in backedges.items() if h <= cont_pc <= j), None)
        if header is None:
            return None
        try:                                            # run THREE body iterations from the concrete header state
            S1 = _run_body_once(tape, cont_pc, header, MEM, R, sp)
            S2 = _run_body_once(tape, cont_pc, header, S1, R, sp)
            S3 = _run_body_once(tape, cont_pc, header, S2, R, sp)
        except Unsupported:
            return None
        r15 = R.get(15, 0)                              # frame locals sit at/above the data-stack top
        carried = [a for a in MEM if isinstance(a, int) and a >= r15
                   and (S1.get(a) != MEM[a] or S2.get(a) != MEM[a] or S3.get(a) != MEM[a])]
        trip = Rb if kind == 'lt' else _add(Rb, 1)      # `<`: 0..R-1 = R iters ; `<=`: 0..R = R+1 iters
        updates = {}; have_counter = False
        for a in carried:
            s0 = MEM[a]
            d1, d2, d3 = _slot_delta(s0, S1[a]), _slot_delta(S1[a], S2[a]), _slot_delta(S2[a], S3[a])
            if any(d is None or (isinstance(d, tuple) and d[0] == 'cmp') for d in (d1, d2, d3)):
                return None
            if d1 == d2 == d3:                          # constant per-iteration increment -> init + trip*delta
                if _concnat(d1) == 1 and s0 == L:
                    have_counter = True                 # a unit-stride counter from L makes the loop run `trip` times
                updates[a] = _add(s0, _mul(trip, d1))   # (counter's own 0 + trip*1 = trip falls out here)
            elif (_concnat(d1), _concnat(d2), _concnat(d3)) == (0, 1, 2):   # increment(k)==k -> Σ_{k<trip}k = g(trip)
                updates[a] = _add(s0, ('f', TRI_ID, trip))
            else:
                return None                             # a delta not (yet) summarizable
        if not have_counter:
            return None
        MEM.update(updates)
        return exit_pc
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
        elif op == 0x0D or op == 0x0E:                   # jz / jnz c, a
            c = reg(tape[pc + 1])
            if isinstance(c, tuple) and c and c[0] == 'cmp':   # a symbolic loop guard -> summarize the loop
                # jz exits (jumps) when c==0 i.e. NOT(L<R); jnz exits on fall-through. Continue = the other edge.
                if op == 0x0D:
                    exit_pc, cont_pc = imm8(pc + 2), pc + 10
                else:
                    exit_pc, cont_pc = pc + 10, imm8(pc + 2)
                nxt = summarize(c, cont_pc, exit_pc)
                if nxt is None:
                    raise Unsupported('loop not in the summarizable linear class')
                pc = nxt
            else:
                c = _concrete(c, 'branch on symbolic value')
                taken = (c == 0) if op == 0x0D else (c != 0)
                pc = imm8(pc + 2) if taken else pc + 10
        elif op == 0x0F or op == 0x10:                   # jlt / jeq a, b, a2
            x, y = reg(tape[pc + 1]), reg(tape[pc + 2])
            if not (isinstance(x, int) and isinstance(y, int)):   # symbolic compare: try bc's boolean idiom
                idiom = _cmp_idiom(tape, pc)
                if idiom is None:
                    raise Unsupported('symbolic comparison outside the recognized boolean idiom')
                rz, lj, iop, fv = idiom
                if iop == 0x0F and fv == 0:                # jlt standard: (x < y)   counter x, bound y
                    R[rz] = ('cmp', 'lt', x, y)
                elif iop == 0x0F and fv == 1:              # jlt swapped: (y <= x)   counter y, bound x  (i<=n)
                    R[rz] = ('cmp', 'le', y, x)
                else:                                      # jeq (==) / swapped (!=) — not a summarizable guard
                    R[rz] = ('cmp', 'eq' if fv == 0 else 'ne', x, y)
                pc = lj
            elif op == 0x0F:
                pc = imm8(pc + 3) if _s64(x) < _s64(y) else pc + 11
            else:
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
