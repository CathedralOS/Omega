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
sys.setrecursionlimit(400000)          # deep-nat traversals (buffer addresses render as s^k chains)

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

# ZZ_CID: a ℤ difference-pair value ('zz', pos, neg) = pos - neg (see beta_symbolic). The observable is mod 256
# and 256 | 2^64, so ℤ arithmetic mod 256 == alpha's mod-2^64 arithmetic mod 256 — this soundly models `sub`.
ZZ_CID = 5
# MN_CID: monus ('mn', a, b) = max(0, a - b), truncated subtraction over ℕ — the BRANCH-FREE trip count
# of a loop whose counter starts at a symbolic value: `i = a; while (i < n)` runs exactly n ∸ a times for ALL
# inputs (a > n gives 0 on the machine and in ℕ alike). Like zz, it is a plain binary constructor to the
# kernel — (data 6 2 0 0), certs by refl — with its MEANING carried by the two differentially-pinned engines.
MN_CID = 6
# SV_CID / SSUM_CID: the INPUT STREAM in the term language — ('sv', t) = the t-th input byte (k 7 t);
# ('ssum', lo, hi) = Σ_{j=lo}^{hi-1} input[j] (k 8 lo hi), the closed form of `acc += read_byte()` loops.
# Fixed-index reads stay (v k). The read POSITION is a virtual frame slot (RDV) so the ordinary summarizer
# machinery (delta +1/iteration, markers) handles the stream with no special-purpose recognizer.
SV_CID = 7
SSUM_CID = 8
# COND/Bxx: CONDITIONAL terms — the meaning of a program that BRANCHES on data. ('cond', b, t, f) selects t
# when b is true, rendered (k 9 b t f) (the kernel accepts arity-3 constructors); the boolean b is one of
# ('blt'|'ble'|'beq'|'bne', L, R), rendered (k 10..13 L R). Comparisons evaluate over ℤ — sound because the
# machine compares the 2^64-wrapped value SIGNED, which agrees with ℤ for |x| < 2^63 (inputs are bytes and
# the fragment's arithmetic stays far below). Like every constructor family: kernel checks refl, meaning
# lives in the two differentially-pinned evaluators.
COND_CID = 9
BOOL_CID = {'blt': 10, 'ble': 11, 'beq': 12, 'bne': 13}
# DIV_CID / MOD_CID: integer division and remainder as OPAQUE binary constructors — ('div', a, b) = a / b and
# ('mod', a, b) = a % b (signed truncated, matching the VM). Like every constructor family: the kernel only
# checks refl on identical terms (both the bytecode and the source derive the SAME div/mod term, so equivalence
# is refl — no division axioms in the trust core), and the two differentially-pinned evaluators carry the
# meaning. The observable is mod 256, and for the gate's non-negative byte inputs trunc = floor = ℕ division.
DIV_CID = 14
MOD_CID = 15
RDV = -8                               # the virtual slot holding the read position (no real slot is negative)
SEGK = -16                             # BMEM key holding fill SEGMENTS: ((base, trip, rdbase), ...) — a copy
                                       # loop's closed form: byte[base+j] = input[rdbase+j] for j < trip
EVK = -24                              # body-run MEM key collecting symbolic-address byte-store EVENTS

def render(t):                         # -> check.beta / prover term syntax
    h = t[0]
    if h == 'z':  return 'z'
    if h == 's':  return '(s %s)' % render(t[1])
    if h == 'v':  return '(v %d)' % t[1]
    if h == 'f':  return '(f %d %s)' % (t[1], render(t[2]))
    if h == 'zz': return '(k %d %s %s)' % (ZZ_CID, render(t[1]), render(t[2]))
    if h == 'mn': return '(k %d %s %s)' % (MN_CID, render(t[1]), render(t[2]))
    if h == 'sv': return '(k %d %s)' % (SV_CID, render(_term(t[1])))
    if h == 'ssum': return '(k %d %s %s)' % (SSUM_CID, render(_term(t[1])), render(_term(t[2])))
    if h == 'cond': return '(k %d %s %s %s)' % (COND_CID, render(t[1]), render(_term(t[2])), render(_term(t[3])))
    if h == 'div': return '(k %d %s %s)' % (DIV_CID, render(_term(t[1])), render(_term(t[2])))
    if h == 'mod': return '(k %d %s %s)' % (MOD_CID, render(_term(t[1])), render(_term(t[2])))
    if h in BOOL_CID: return '(k %d %s %s)' % (BOOL_CID[h], render(_term(t[1])), render(_term(t[2])))
    return '(%s %s %s)' % (h, render(t[1]), render(t[2]))

def evaluate(t, env):                  # concrete integer value (ℤ; the gate observes it mod 256)
    h = t[0]
    if h == 'z':  return 0
    if h == 's':  return 1 + evaluate(t[1], env)
    if h == 'v':  return env[t[1]]
    if h == 'zz': return evaluate(t[1], env) - evaluate(t[2], env)
    if h == 'mn': return max(0, evaluate(t[1], env) - evaluate(t[2], env))
    if h == 'sv': return env['in'][evaluate(_term(t[1]), env)]
    if h == 'ssum': return sum(env['in'][evaluate(_term(t[1]), env):evaluate(_term(t[2]), env)])
    if h == 'cond': return evaluate(_term(t[2]), env) if evaluate(t[1], env) else evaluate(_term(t[3]), env)
    if h == 'div': return _trunc_div(evaluate(_term(t[1]), env), evaluate(_term(t[2]), env))
    if h == 'mod':
        a = evaluate(_term(t[1]), env); b = evaluate(_term(t[2]), env)
        return a - _trunc_div(a, b) * b
    if h == 'blt': return 1 if evaluate(_term(t[1]), env) < evaluate(_term(t[2]), env) else 0
    if h == 'ble': return 1 if evaluate(_term(t[1]), env) <= evaluate(_term(t[2]), env) else 0
    if h == 'beq': return 1 if evaluate(_term(t[1]), env) == evaluate(_term(t[2]), env) else 0
    if h == 'bne': return 1 if evaluate(_term(t[1]), env) != evaluate(_term(t[2]), env) else 0
    if h == 'f':                       # a user-function recurrence; TRI_ID is the triangular sum g(n)=Σ_{j<n} j
        if t[1] != TRI_ID:
            raise Unsupported('unknown recurrence fun %d' % t[1])
        a = evaluate(t[2], env)
        return a * (a - 1) // 2
    if h == 'p':  return evaluate(t[1], env) + evaluate(t[2], env)
    return evaluate(t[1], env) * evaluate(t[2], env)     # m

# ---- the dual value model: Python int = concrete, tuple = symbolic Peano term (or a ('zz',pos,neg) ℤ pair) --
def _term(v):                          # coerce a value to a Peano term (concretes become s^k z)
    if isinstance(v, tuple) and v and v[0] == 'cmp':
        raise Unsupported('a comparison used as a value (stored / arithmetic boolean) — not modelled yet')
    return nat(v) if isinstance(v, int) else v

def _s64(x):                           # a concrete 64-bit word as signed
    return x - (1 << 64) if x >= (1 << 63) else x

_ZERO = ('z',)
def _is_zz(v):  return isinstance(v, tuple) and v[0] == 'zz'
def _as_zz(v):  return (v[1], v[2]) if _is_zz(v) else (_term(v), _ZERO)
def _padd(x, y):
    if x == _ZERO: return y
    if y == _ZERO: return x
    return ('p', x, y)
def _pmul(x, y):
    if x == _ZERO or y == _ZERO: return _ZERO
    if x == ('s', _ZERO): return y
    if y == ('s', _ZERO): return x
    return ('m', x, y)

def _add(a, b):
    if _is_zz(a) or _is_zz(b):
        (pa, na), (pb, nb) = _as_zz(a), _as_zz(b)
        return ('zz', _padd(pa, pb), _padd(na, nb))
    if isinstance(a, int) and isinstance(b, int):
        return (a + b) & MASK
    return ('p', _term(a), _term(b))

def _mul(a, b):
    if _is_zz(a) or _is_zz(b):          # (pa-na)(pb-nb) = (pa·pb + na·nb) - (pa·nb + na·pb)
        (pa, na), (pb, nb) = _as_zz(a), _as_zz(b)
        return ('zz', _padd(_pmul(pa, pb), _pmul(na, nb)), _padd(_pmul(pa, nb), _pmul(na, pb)))
    if isinstance(a, int) and isinstance(b, int):
        return (a * b) & MASK
    return ('m', _term(a), _term(b))

def _sub(a, b):
    if isinstance(a, int) and isinstance(b, int):
        if a >= b:
            return (a - b) & MASK      # exact: concrete address/offset arithmetic, or a non-underflowing literal
        # a < b: a DATA underflow (addresses never underflow — a base is always ≥ its offset). Model the true
        # ℤ result as a small difference pair so it renders (0-1 → (k 5 z (s z)) = 255) instead of 2^64-1.
        (pa, na), (pb, nb) = _as_zz(a), _as_zz(b)
        return ('zz', _padd(pa, nb), _padd(na, pb))
    (pa, na), (pb, nb) = _as_zz(a), _as_zz(b)      # symbolic: ℤ difference pair  (pa+nb) - (na+pb)
    return ('zz', _padd(pa, nb), _padd(na, pb))

def _concrete(v, why):
    if not isinstance(v, int):
        raise Unsupported(why)
    return v

def _trunc_div(a, b):                  # signed division truncated toward zero (matches the VM's trunc_div)
    if b == 0:
        raise Unsupported('division by zero')   # the machine traps (SIGILL); not modelled
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q

def _divmod(op, a, b):                  # op in {'div','mod'}: opaque constructor over the operands. Both concrete
    if isinstance(a, int) and isinstance(b, int):   # -> fold now (matches the VM); else an opaque symbolic term
        q = _trunc_div(_s64(a), _s64(b))
        return (q if op == 'div' else _s64(a) - q * _s64(b)) & MASK
    if _is_zz(a) or _is_zz(b):
        raise Unsupported('div/mod on a signed (ℤ-pair) operand — not modelled yet')
    return (op, _term(a), _term(b))

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

def _run_body_once(tape, backedges, start_pc, header, MEM, R, sp, depth=0, brcell=None):
    """Speculatively execute ONE loop iteration from start_pc until pc jumps back to `header`; return the
    resulting memory. Operates on copies, so the caller's state is untouched. Control must be concrete —
    EXCEPT an inner loop guard: an inner loop with a concrete bound unrolls right here, and one with a
    SYMBOLIC bound is summarized recursively via _summarize, its closed forms (over this run's slot markers)
    flowing into the outer deltas. This is the EXACT per-iteration transition."""
    MEM = dict(MEM); R = dict(R); pc = start_pc; steps = 0
    if brcell is None:
        brcell = [False]
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
        elif op == 0x0A:
            a = _concrete(reg(tape[pc + 2]), 'load'); v = MEM.get(a, 0)
            if v == ('poison',):
                raise Unsupported('read of a slot dropped by loop summarization')
            R[tape[pc + 1]] = v; pc += 3
        elif op == 0x0B: a = _concrete(reg(tape[pc + 1]), 'store'); MEM[a] = reg(tape[pc + 2]); pc += 3
        elif op in (0x0D, 0x0E):                        # jz / jnz
            c = reg(tape[pc + 1])
            if isinstance(c, tuple) and c and c[0] in ('blt', 'ble', 'beq', 'bne'):
                if op == 0x0D:                                  # an INNER symbolic loop guard: recurse
                    exit_pc, cont_pc = _le8(tape, pc + 2), pc + 10
                else:
                    exit_pc, cont_pc = pc + 10, _le8(tape, pc + 2)
                kind = {'blt': 'lt', 'ble': 'le', 'beq': 'eq', 'bne': 'ne'}[c[0]]
                nxt = _summarize(tape, backedges, ('cmp', kind, c[1], c[2]), cont_pc, exit_pc, MEM, R, sp, depth + 1)
                if nxt is None:                             # an IF-DIAMOND inside the body: fork both paths
                    if depth >= 4:                          # to the header and merge post-states pointwise —
                        raise Unsupported('too many branches inside a summarized loop body')
                    brcell[0] = True
                    MT = _run_body_once(tape, backedges, cont_pc, header, MEM, dict(R), sp, depth + 1, brcell)
                    MF = _run_body_once(tape, backedges, exit_pc, header, MEM, dict(R), sp, depth + 1, brcell)
                    merged = {}                             # a slot differing across paths becomes (cond c ..)
                    for kk in set(MT) | set(MF):
                        vT, vF = MT.get(kk, MEM.get(kk, 0)), MF.get(kk, MEM.get(kk, 0))
                        merged[kk] = vT if vT == vF else ('cond', c, vT, vF)
                    return merged
                pc = nxt
            else:
                brcell[0] = True                            # a concrete data branch: the fast path's 3-run
                c = _concrete(c, 'symbolic branch inside a summarized loop body')   # probe can be fooled by
                taken = (c == 0) if op == 0x0D else (c != 0)                        # short-horizon uniformity
                pc = _le8(tape, pc + 2) if taken else pc + 10
        elif op in (0x0F, 0x10):                        # jlt / jeq — concrete compares branch; a symbolic
            x, y = reg(tape[pc + 1]), reg(tape[pc + 2])  # one becomes a boolean via bc's idiom (as in symexec)
            if isinstance(x, int) and isinstance(y, int):
                brcell[0] = True
                hit = (_s64(x) < _s64(y)) if op == 0x0F else (x == y)
                pc = _le8(tape, pc + 3) if hit else pc + 11
            else:
                idiom = _cmp_idiom(tape, pc)
                if idiom is None:
                    raise Unsupported('symbolic comparison outside the recognized boolean idiom')
                rz, lj, iop, fv = idiom
                if iop == 0x0F and fv == 0:
                    R[rz] = ('blt', x, y)   # sides stay RAW (ints keep summarize's slot matching)
                elif iop == 0x0F and fv == 1:
                    R[rz] = ('ble', y, x)
                else:
                    R[rz] = ('beq' if fv == 0 else 'bne', x, y)
                pc = lj
        elif op == 0x11:                                # read d — a stream element at the current position
            cur = MEM.get(RDV, 0)
            R[tape[pc + 1]] = ('v', cur) if isinstance(cur, int) else ('sv', cur)
            MEM[RDV] = _add(cur, 1) if not isinstance(cur, int) else cur + 1
            pc += 2
        elif op == 0x09:                                # storeb inside a body: a SYMBOLIC address is a fill
            a2 = reg(tape[pc + 1])                      # EVENT (byte[base+ctr] = v, judged by the summarizer);
            if isinstance(a2, int):                     # concrete in-body byte stores stay refused (slice)
                raise Unsupported('concrete byte store inside a summarized loop body')
            MEM[EVK] = MEM.get(EVK, ()) + ((a2, reg(tape[pc + 2])),)
            pc += 3
        elif op == 0x13:                                # call a — push the return offset, enter the callee
            sp -= 8; MEM[sp] = pc + 9; pc = _le8(tape, pc + 1)
        elif op == 0x14:                                # ret — pop the return offset
            pc = _concrete(MEM.get(sp, 0), 'corrupt return address in a summarized loop body'); sp += 8
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
    if isinstance(d, tuple) and d[0] in ('p', 'm'):     # fold additive/multiplicative trees of concretes
        l, r = _concnat(d[1]), _concnat(d[2])           # (a peeled multi-read position delta is (p (s z) (s z)))
        if l is None or r is None:
            return None
        return l + r if d[0] == 'p' else l * r
    return None

def _canon(x):                         # canonicalize a concrete coefficient to an int so 0/1 simplify identically
    c = _concnat(x)
    return c if c is not None else x

def _is_negone(d):                     # is delta d the ℤ pair -1 (pos 0, neg 1)? the down-counter's stride
    return isinstance(d, tuple) and d[0] == 'zz' and _canon(d[1]) == 0 and _canon(d[2]) == 1

def _series_closed(init, a0, a1, trip):
    """init + a0·trip + a1·g(trip), canonical (same construction as beta_symbolic._series_closed → refl-equal)."""
    def scaled(coef, base):
        if coef == 0: return None
        if coef == 1: return base
        return ('m', base, _term(coef))
    r = _term(init)
    for p in (scaled(a0, trip), scaled(a1, ('f', TRI_ID, trip))):
        if p is not None:
            r = ('p', r, p)
    return r

def _down_series(p0, n0, a0p, a1p, a0n, a1n, trip):
    """Closed ℤ pair for a DOWN-counting loop (counter value n-k at iteration k, trip = n). A pair-delta with
    components a0p + a1p·i and a0n + a1n·i sums, after i ↦ n-k, to (a0x + a1x·n)·t - a1x·g(t) per component —
    the linear part joins the invariant coefficient and the triangular part FLIPS SIGN, crossing to the other
    component. Same recipe as beta_symbolic._down_series so the forms stay byte-identical."""
    return ('zz', _series_closed(p0, _canon(_sum2(a0p, _scale2(a1p, trip))), _canon(a1n), trip),
                  _series_closed(n0, _canon(_sum2(a0n, _scale2(a1n, trip))), _canon(a1p), trip))

# ---- general linear-in-counter delta extraction (placeholder path, mirrors beta_symbolic) --------------
# When the finite differences can't classify a delta (a·i, a+i, …), run one body iteration with every frame
# slot set to a ('slot', addr) placeholder, read each accumulator's delta as a symbolic term over those
# markers, substitute the loop-INVARIANT slots' placeholders back to their values, and decompose what remains
# over the COUNTER slot's marker into a0 + a1·counter.
def _mentions_slot(t):
    if isinstance(t, tuple):
        if t[0] == 'slot':
            return True
        if t[0] in ('s', 'p', 'm', 'f', 'zz', 'mn', 'sv', 'ssum', 'blt', 'ble', 'beq', 'bne', 'cond'):
            return any(_mentions_slot(x) for x in t[1:])
    return False

def _has_stream(t):                    # does a stream term (sv / ssum) occur anywhere inside `t`?
    if isinstance(t, tuple):
        if t[0] in ('sv', 'ssum'):
            return True
        if t[0] in ('s', 'p', 'm', 'f', 'zz', 'mn', 'blt', 'ble', 'beq', 'bne', 'cond'):
            return any(_has_stream(x) for x in t[1:])
    return False

def _split_stream(d, rdmark):
    """d ≡ rest + coef·(sv rdmark) -> (rest, coef); coef None when d has no stream part; (None, None) when the
    stream part is not linearly separable (e.g. (sv rd)·(sv rd), or a read under a non-invariant factor).
    Shared shape in both engines so the closed forms stay byte-identical."""
    sv = ('sv', rdmark)
    if d == sv:
        return (0, 1)
    if isinstance(d, tuple):
        if d[0] == 'm':
            if d[1] == sv and not _has_stream(d[2]):
                return (0, d[2])
            if d[2] == sv and not _has_stream(d[1]):
                return (0, d[1])
        if d[0] == 'p':
            ls, rs = _has_stream(d[1]), _has_stream(d[2])
            if ls and rs:
                return (None, None)
            if not ls and not rs:
                return (d, None)
            side = 1 if ls else 2
            rest, coef = _split_stream(d[side], rdmark)
            if coef is None and rest is None:
                return (None, None)
            other = d[3 - side]
            if coef is None:
                return (d, None)
            combined = other if rest == 0 else ('p', _term(rest), _term(other)) if side == 1 else ('p', _term(other), _term(rest))
            return (combined, coef)
    return (d, None) if not _has_stream(d) else (None, None)

def _read_sum(rest_closed, base, trip, coef=1, width=1):
    """rest-series + coef·Σ input[base .. base + width·trip). The upper end is exactly the read POSITION's
    own series closure (delta `width` per iteration), so the forms stay byte-identical at width 1."""
    ssum = ('ssum', _term(base), _series_closed(base, width, 0, trip))
    if _canon(coef) != 1:
        ssum = ('m', ssum, _term(coef))
    return ('p', _term(rest_closed), ssum)

def _stream_offsets(d, rdmark):
    """d ≡ rest + Σ_j (sv rdmark+off_j), every read atom coefficient 1 -> (rest, sorted offsets);
    (None, None) if any stream part is not a bare offset atom. Offsets are per-iteration read positions."""
    def atom_off(t):
        if t == ('sv', rdmark):
            return 0
        if isinstance(t, tuple) and t[0] == 'sv' and isinstance(t[1], tuple):
            d = _peel(t[1], rdmark)                     # index = rdmark + off, possibly a left-nested chain
            return _concnat(d) if d is not None else None
        return None
    if not _has_stream(d):
        return (d, [])
    o = atom_off(d)
    if o is not None:
        return (0, [o])
    if isinstance(d, tuple) and d[0] == 'p':
        lr, lo = _stream_offsets(d[1], rdmark)
        rr, ro = _stream_offsets(d[2], rdmark)
        if lo is None or ro is None or None in (lo or []) or None in (ro or []):
            return (None, None)
        rest = rr if lr == 0 else lr if rr == 0 else ('p', _term(lr), _term(rr))
        return (rest, lo + ro)
    return (None, None)

def _component_closed(init, comp_rest_dec, coef, base, trip, off):
    """Close one ℤ-pair component: the ordinary series over its rest (offset-folded), plus an optional
    coefficiented stream sum. Identical construction in both engines."""
    rest_closed = _series_closed(init, _canon(_sum2(comp_rest_dec[0], _scale2(comp_rest_dec[1], off))),
                                 _canon(comp_rest_dec[1]), trip)
    if coef is None:
        return rest_closed
    return _read_sum(rest_closed, base, trip, coef)

def _has_zz(t):                        # does a zz pair occur anywhere inside term `t`?
    if isinstance(t, tuple):
        if t[0] == 'zz':
            return True
        if t[0] in ('s', 'p', 'm', 'f', 'mn', 'blt', 'ble', 'beq', 'bne', 'cond'):
            return any(_has_zz(x) for x in t[1:])
    return False

def _occurs(t, ph):                    # does the exact marker `ph` occur in term `t`?
    if t == ph:
        return True
    return isinstance(t, tuple) and t[0] in ('s', 'p', 'm', 'f', 'zz', 'mn', 'sv', 'ssum', 'blt', 'ble', 'beq', 'bne', 'cond') and any(_occurs(x, ph) for x in t[1:])

def _subst_slots(t, MEM, invariant):   # ('slot', addr) -> MEM[addr] for loop-invariant addrs; recurse elsewhere
    if isinstance(t, tuple):
        if t[0] == 'slot':
            return MEM[t[1]] if t[1] in invariant else t
        if t[0] in ('s', 'sv'):
            return (t[0], _subst_slots(t[1], MEM, invariant))
        if t[0] in ('p', 'm', 'zz', 'mn', 'ssum', 'blt', 'ble', 'beq', 'bne', 'cond'):
            return (t[0],) + tuple(_subst_slots(x, MEM, invariant) for x in t[1:])
        if t[0] == 'f':
            return ('f', t[1], _subst_slots(t[2], MEM, invariant))
    return t

def _sum2(a, b):
    if a == 0: return b
    if b == 0: return a
    if isinstance(a, int) and isinstance(b, int): return a + b
    return ('p', _term(a), _term(b))

def _scale2(coef, x):
    if x == 0 or coef == 0: return 0
    if isinstance(coef, int) and isinstance(x, int): return coef * x
    if x == 1: return coef
    if coef == 1: return x
    return ('m', _term(coef), _term(x))

def _mentions_marked(t, marked):       # does term `t` mention any marker in the `marked` set?
    if t in marked:
        return True
    if isinstance(t, tuple) and t[0] in ('s', 'p', 'm', 'f', 'zz', 'mn', 'sv', 'ssum', 'blt', 'ble', 'beq', 'bne', 'cond'):
        return any(_mentions_marked(x, marked) for x in t[1:])
    return False

def _lin_decompose(delta, ctr, moved):
    """delta = a0 + a1·ctr, a0/a1 free of every marker in `moved` (THIS loop's changing slots); None if not
    linear in the counter. Markers NOT in `moved` are someone else's (an OUTER loop's placeholder context in a
    nested summarization) and are loop-invariant HERE — they pass through as opaque constants, exactly as
    beta_symbolic's decompose treats non-loop-var markers."""
    def dec(t):
        if t == ctr:
            return (0, 1)
        if not _mentions_marked(t, moved):
            return (t, 0)                                # invariant in THIS loop (outer markers stay opaque)
        if isinstance(t, tuple):
            if t[0] == 'p':
                l, r = dec(t[1]), dec(t[2])
                return None if l is None or r is None else (_sum2(l[0], r[0]), _sum2(l[1], r[1]))
            if t[0] == 'm':
                inv, oth = ((t[1], t[2]) if not _mentions_marked(t[1], moved) else
                            (t[2], t[1]) if not _mentions_marked(t[2], moved) else (None, None))
                if inv is None:
                    return None                          # counter·counter or counter·another-slot: non-linear
                d = dec(oth)
                return None if d is None else (_scale2(inv, d[0]), _scale2(inv, d[1]))
        return None
    return dec(delta)

def _peel(s1, s0t):
    """s1 = an additive spine containing the entry term s0t -> the spine with s0t removed (the per-iteration
    delta, preserving the tree's shape so both engines build identical terms). Left-first when ambiguous —
    accumulator chains grow leftward, so the entry sits on the leftmost spine. An UNROLLED inner concrete
    loop leaves exactly this shape: ((s0 + a) + a) + a -> (a + a) + a."""
    if s1 == s0t:
        return 0
    if isinstance(s1, tuple) and s1[0] == 'p':
        side = 1 if _occurs(s1[1], s0t) else 2 if _occurs(s1[2], s0t) else 0
        if side:
            d = _peel(s1[side], s0t)
            other = s1[3 - side]
            if d is None:
                return None
            if d == 0:
                return other
            return ('p', d, other) if side == 1 else ('p', other, d)
    return None

def _slot_delta(s0, s1):
    """per-iteration increment given a slot's entry value s0 and its value s1 after one body run (s1 = s0 + d).
    A DECREASING slot (concrete s1 < s0, or s1 a zz pair) never yields a plain delta here: the concrete case
    would otherwise wrap to a 2^64-ish coefficient whose Peano rendering is unbuildable, so both are pushed to
    the caller's general placeholder path. There a zz s1 = ('zz', s0 + Dp, N) IS extractable — the pos/neg
    components follow independent additive recurrences — as the pair-delta ('zz', Dp, N)."""
    if isinstance(s0, int) and isinstance(s1, int):
        return (s1 - s0) if s1 >= s0 else None
    if s1 == s0:
        return 0
    if isinstance(s1, tuple) and s1[0] == 'p':          # s1 = (p s0 d) / (p d s0), or a deeper additive spine
        if s1[1] == _term(s0):
            return s1[2]
        if s1[2] == _term(s0):
            return s1[1]
        d = _peel(s1, _term(s0))
        if d is not None:
            return d
    if isinstance(s1, tuple) and s1[0] == 'zz' and not _occurs(s1[2], s0):
        dp = _slot_delta(s0, s1[1])
        if dp is not None and not (isinstance(dp, tuple) and dp[0] == 'zz'):
            return ('zz', dp, s1[2])
    if isinstance(s1, tuple) and s1[0] == 'cond' and not _occurs(s1[1], s0):
        dT, dF = _slot_delta(s0, s1[2]), _slot_delta(s0, s1[3])   # a conditional post-value: per-branch deltas
        if dT is not None and dF is not None:
            return ('cond', s1[1], dT, dF)
    return None

def _summarize(tape, backedges, cond, cont_pc, exit_pc, MEM, R, sp, depth=0, BMEM=None):
    """cond = ('cmp', kind, L, R) guards a loop that continues to cont_pc and exits to exit_pc. Read the
    per-iteration transition off ONE speculative body run, recognize a unit-stride counter from 0 (or a
    down/!=-guarded equivalent) and linear-in-counter accumulator deltas, and replace the loop by each
    accumulator's closed form. Mutates the GIVEN MEM to the post-loop state; returns exit_pc, or None if the
    loop is outside the summarizable class (caller then bails). Module-level so _run_body_once can recurse
    into it: an INNER loop with a symbolic bound met during a body run is summarized in place, its closed
    forms (over the outer run's slot markers) flowing into the outer deltas."""
    if depth > 8:
        return None                                     # nesting depth guard (mutual recursion backstop)
    kind, L, Rb = cond[1], cond[2], cond[3]
    if kind == 'ne':
        # Over ℕ with a unit-stride counter, != is <: one side must be the literal 0 — `i != n` from 0
        # (L is the counter's concrete entry 0) hits n exactly, and `i != 0` maps to the 0 < i down shape.
        # The counter checks below enforce exactly the entry/stride conditions the exact-hit needs; a
        # stride that could SKIP the bound (the machine diverges) fails the ±1 checks and refuses.
        if L == 0:
            kind = 'lt'
        elif Rb == 0:
            kind, L, Rb = 'lt', 0, L
    if kind not in ('lt', 'le'):
        return None                                 # `counter < bound` / `counter <= bound`
    if isinstance(L, tuple) and L[0] in ('zz', 'mn', 'cmp'):
        return None                                 # a ℤ-pair / monus / boolean start value: later
    off = _canon(L) if not isinstance(L, int) else L    # the counter's START (0 keeps today's forms)
    header = next((h for h, j in backedges.items() if h <= cont_pc <= j), None)
    if header is None:
        return None
    br = [False]                                    # run THREE body iterations from the concrete header state;
    try:                                            # if any crossed a data branch, the finite-difference probe
        S1 = _run_body_once(tape, backedges, cont_pc, header, MEM, R, sp, depth, br)   # is unreliable (short-
        S2 = _run_body_once(tape, backedges, cont_pc, header, S1, R, sp, depth, br)    # horizon uniformity) and
        S3 = _run_body_once(tape, backedges, cont_pc, header, S2, R, sp, depth, br)    # the marker path decides
    except Unsupported:
        S1 = None                                   # the concrete probe failed (e.g. a fill store): the
        br[0] = True                                # marker path alone decides
    r15 = R.get(15, 0)                              # frame locals sit at/above the data-stack top
    carried = [] if S1 is None else [a for a in MEM if isinstance(a, int) and (a >= r15 or a == RDV)
               and (S1.get(a) != MEM[a] or S2.get(a) != MEM[a] or S3.get(a) != MEM[a])]
    hi = Rb if kind == 'lt' else _add(Rb, 1)        # exclusive upper end: R (<) or R+1 (<=)
    # from 0: trip = hi (the existing forms). From a symbolic/nonzero start: trip = hi ∸ start — MONUS, the
    # branch-free trip count (start > hi runs 0 times on the machine and 0 = hi ∸ start in ℕ alike).
    trip = hi if off == 0 else ('mn', _term(hi), _term(L))
    # FAST PATH — finite differences over the 3 iterations (invariant + pure-Σi deltas)
    updates = {}; have_counter = False; general = br[0]
    for a in carried:
        s0 = MEM[a]
        d1, d2, d3 = _slot_delta(s0, S1[a]), _slot_delta(S1[a], S2[a]), _slot_delta(S2[a], S3[a])
        if d1 == d2 == d3 and d1 is not None and not (isinstance(d1, tuple) and d1[0] in ('cmp', 'zz')):
            if _concnat(d1) == 1 and s0 == L:
                have_counter = True                 # a unit-stride counter from L makes the loop run `trip` times
            updates[a] = _series_closed(s0, _canon(d1), 0, trip)
        elif (_concnat(d1), _concnat(d2), _concnat(d3)) == (0, 1, 2):   # increment(k)==k -> Σ_{k<trip}k = g(trip)
            updates[a] = _series_closed(s0, 0, 1, trip)
        else:
            general = True; break                   # a·i / a+i / … -> the general placeholder path below
    if not general:
        if not have_counter:
            return None
        MEM.update(updates)
        return exit_pc
    # GENERAL PATH — one placeholder iteration: read each δ as a term over ('slot',*) markers, then decompose
    frame = [a for a in MEM if isinstance(a, int) and (a >= r15 or a == RDV)]
    PMEM = dict(MEM)
    for a in frame:
        PMEM[a] = ('slot', a)
    try:
        PS = _run_body_once(tape, backedges, cont_pc, header, PMEM, R, sp, depth)
    except Unsupported:
        return None
    raw = {}
    rewrite = set()                                 # REWRITE slots: fully overwritten each iteration (a call
    for a in frame:                                 # temp, t = a*i, …) — no additive delta exists. They are
        d = _slot_delta(('slot', a), PS.get(a, ('slot', a)))    # dropped (poisoned) post-loop; any OTHER
        if isinstance(d, tuple) and d[0] == 'cmp':  # delta that reads their stale value refuses below.
            return None
        if d is None:
            rewrite.add(a)
            continue
        raw[a] = d
    fill_events = PS.get(EVK, ())
    if fill_events:                                 # a COPY loop: one byte[base+ctr] = read_byte() / iteration
        if BMEM is None or len(fill_events) != 1:
            return None
        fa, fv = fill_events[0]
        if fv != ('sv', ('slot', RDV)) or _canon(raw.get(RDV)) != 1 or not isinstance(MEM.get(RDV), int):
            return None
    invariant = {a for a in frame if raw.get(a) == 0}   # slots unchanged this iteration are loop-invariant
    moved = [a for a in frame if a in raw and raw[a] != 0]
    down = False
    counters = [a for a in moved if _canon(raw[a]) == 1 and MEM[a] == L]
    if not counters:
        # DOWN-count: guard `0 < i` (L the concrete 0, Rb the counter's symbolic entry value) with a slot
        # stepping by the ℤ pair -1 — it drains I, I-1, …, 1, so exactly trip = Rb iterations. `0 <= i`
        # with -1 never terminates and is not recognized. A ℤ-pair entry value can't be a trip count yet.
        if kind != 'lt' or off != 0 or (isinstance(Rb, tuple) and Rb[0] == 'zz'):
            return None
        counters = [a for a in moved if _is_negone(raw[a]) and MEM[a] == Rb]
        if not counters:
            return None
        down = True
    ctr = ('slot', counters[0])
    movedset = {('slot', a) for a in moved}
    rewriteset = {('slot', a) for a in rewrite}
    updates = {}
    for a in rewrite:
        updates[a] = ('poison',)                    # a post-loop LOAD of a dropped slot refuses
    for a in moved:
        d = raw[a]
        if isinstance(d, tuple) and d[0] == 'zz':   # subtracting accumulator: pos/neg components follow
            # each pair component may carry its OWN stream part: acc -= read puts the Σ on the NEG side
            comps = []
            for raw_comp in (d[1], d[2]):
                rest, coef = _split_stream(raw_comp, ('slot', RDV))
                if rest is None and coef is None:
                    return None                     # not linearly separable (read·read, …)
                if coef is not None:
                    if _canon(raw.get(RDV)) != 1 or not isinstance(MEM.get(RDV), int) or down:
                        return None                 # stride-1 reads, concrete base, up-counting only
                    coef = _subst_slots(coef, MEM, invariant) if isinstance(coef, tuple) else coef
                    if (_has_stream(coef) or _has_zz(coef) or _mentions_marked(coef, movedset)
                            or _mentions_marked(coef, rewriteset) or _occurs(coef, ('slot', RDV))):
                        return None                 # the read's coefficient must be loop-invariant
                rest_s = _subst_slots(rest, MEM, invariant) if rest != 0 else 0
                if rest_s != 0 and (_has_stream(rest_s) or _has_zz(rest_s)
                                    or _mentions_marked(rest_s, rewriteset) or _occurs(rest_s, ('slot', RDV))):
                    return None
                dec = (0, 0) if rest_s == 0 else _lin_decompose(rest_s, ctr, movedset)
                if dec is None:
                    return None
                comps.append((dec, coef))
            (dp, pcoef), (dn, ncoef) = comps
            p0, n0 = _as_zz(MEM[a])
            if down:                                # i ↦ n-k: linear parts fold into the invariant
                updates[a] = _down_series(p0, n0, dp[0], dp[1], dn[0], dn[1], trip)
            else:                                   # coefficient, g cross-terms swap components; a start
                updates[a] = ('zz', _component_closed(p0, dp, pcoef, MEM.get(RDV), trip, off),
                                    _component_closed(n0, dn, ncoef, MEM.get(RDV), trip, off))
            continue
        if _has_stream(d) or _occurs(d, ('slot', RDV)):     # δ = rest + coef·read: an invariant-coefficient
            R = _canon(raw.get(RDV))                # stream sum plus an ordinary series
            if not isinstance(R, int) or R < 1 or not isinstance(MEM.get(RDV), int):
                return None                         # a fixed per-iteration read count, from a concrete base
            rest, coef = _split_stream(d, ('slot', RDV))
            width = 1
            if coef is None:                        # not a single coefficiented read: try the WIDE shape —
                rest, offs = _stream_offsets(d, ('slot', RDV))      # the acc must consume ALL R reads, each
                if offs is None or sorted(offs) != list(range(R)):  # once -> Σ over base..base+R·trip
                    return None
                coef, width = 1, R
            elif R != 1:
                return None                         # one-of-many reads: a STRIDED sum — refused
            coef_s = _subst_slots(coef, MEM, invariant) if isinstance(coef, tuple) else coef
            if (_has_stream(coef_s) or _has_zz(coef_s) or _mentions_marked(coef_s, movedset)
                    or _occurs(coef_s, ('slot', RDV))):
                return None                         # the read's coefficient must be loop-invariant
            if rest == 0:
                dec = (0, 0)
            else:
                rest_s = _subst_slots(rest, MEM, invariant)
                if _has_stream(rest_s) or _has_zz(rest_s):
                    return None
                dec = _lin_decompose(rest_s, ctr, movedset)
                if dec is None or (down and _canon(dec[1]) != 0):
                    return None
            if down and _canon(dec[1]) != 0:
                return None                         # a counter-dependent rest under a down-counter
            rest_closed = _series_closed(MEM[a], _canon(_sum2(dec[0], _scale2(dec[1], off))), _canon(dec[1]), trip)
            updates[a] = _read_sum(rest_closed, MEM[RDV], trip, coef_s, width)
            continue
        sub = _subst_slots(d, MEM, invariant)       # δ = a0 + a1·counter
        if _has_zz(sub):
            return None                             # invariant zz feeding a plain delta: beta distributes
        if _mentions_marked(sub, rewriteset):
            return None                             # the delta reads a rewrite slot's stale value
        dec = _lin_decompose(sub, ctr, movedset)
        if dec is None:
            return None                             # δ not linear in the counter (i·i, cross-accumulator, …)
        if down and _canon(dec[1]) != 0:            # counter-dependent plain δ under a down-counter:
            p0, n0 = _as_zz(MEM[a])                 # the -a1·g(t) cross-term makes the result a ℤ pair
            updates[a] = _down_series(p0, n0, dec[0], dec[1], 0, 0, trip)
            continue
        updates[a] = _series_closed(MEM[a], _canon(_sum2(dec[0], _scale2(dec[1], off))), _canon(dec[1]), trip)
    if fill_events:
        if down or off != 0:
            return None                             # copy loops: up-counting from 0 (slice 1)
        pb = _peel(fill_events[0][0], ctr)          # store address must be exactly base + counter
        base = _concnat(pb) if pb is not None else None
        if base is None:
            return None
        segs = BMEM.get(SEGK, ())
        if any(b0 + 512 > base and base + 512 > b0 for (b0, t0, r0) in segs) \
                or any(isinstance(k2, int) and base <= k2 < base + 512 for k2 in BMEM):
            return None                             # overlapping segments / prior byte writes: refused
        BMEM[SEGK] = segs + ((base, trip, MEM[RDV]),)
    MEM.update(updates)
    return exit_pc

def symexec(tape):
    """Symbolically execute an Alpha tape. Returns (output_term, n_inputs), where inputs are (v 0)..(v n-1)
    in `read` order (plus stream elements past a read-loop). Raises Unsupported on anything outside the
    modelled fragment. Data-dependent LOOPS summarize; data-dependent BRANCHES fork into conditional terms."""
    ncell = [0]
    out = _exec(tape, _back_edges(tape), 0, {}, {}, {}, 0x04000000, ncell, 0)
    return out, ncell[0]

def _exec(tape, backedges, pc, R, MEM, BMEM, sp, ncell, fork_depth):
    """Run from pc to halt/write; return the RESULT term. A symbolic branch that is not a summarizable loop
    FORKS: both paths run to completion on copied state and the result is (cond b then else) — no join
    detection. Reads on each path number consecutively from the fork point, matching the machine's actual
    per-path read order; the arity cell tracks the max across paths."""
    def reg(i):
        return R.get(i, 0)
    steps = 0
    def imm8(at):
        return int.from_bytes(tape[at:at + 8], 'little')

    def summarize(cond, cont_pc, exit_pc):
        return _summarize(tape, backedges, cond, cont_pc, exit_pc, MEM, R, sp, fork_depth, BMEM)
    while True:
        steps += 1
        if steps > 500000:
            raise Unsupported('step budget exceeded (a data-independent loop?)')
        op = tape[pc]
        if op == 0x00:                                   # halt d
            return _term(reg(tape[pc + 1]))
        elif op == 0x12:                                 # write s -> the program's output
            return _term(reg(tape[pc + 1]))
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
            d = tape[pc + 1]
            R[d] = _divmod('div' if op == 0x06 else 'mod', reg(d), reg(tape[pc + 2])); pc += 3
        elif op == 0x0A:                                 # load d, s  (word) — concrete address
            a = _concrete(reg(tape[pc + 2]), 'load from symbolic address')
            if BMEM and any(a <= bb < a + 8 for bb in BMEM):
                raise Unsupported('word access aliases a byte store')
            v = MEM.get(a, 0)
            if v == ('poison',):
                raise Unsupported('read of a slot dropped by loop summarization')
            R[tape[pc + 1]] = v; pc += 3
        elif op == 0x0B:                                 # store d, s (word) — concrete address
            a = _concrete(reg(tape[pc + 1]), 'store to symbolic address')
            if BMEM and any(a <= bb < a + 8 for bb in BMEM):
                raise Unsupported('word access aliases a byte store')
            MEM[a] = reg(tape[pc + 2]); pc += 3
        elif op == 0x08:                                 # loadb d, s — a byte read at a concrete address.
            a = _concrete(reg(tape[pc + 2]), 'byte load from symbolic address')
            if any(isinstance(w, int) and w <= a < w + 8 for w in MEM):
                raise Unsupported('byte access aliases a word slot')
            v = None
            for (b0, t0, r0) in BMEM.get(SEGK, ()):  # a fill segment: byte[b0+j] = input[r0+j] for j < t0
                j = a - b0
                if 0 <= j < 512:
                    v = ('cond', ('blt', j, t0), ('sv', r0 + j), BMEM.get(a, tape[a] if a < len(tape) else 0))
                    break
            if v is None:
                v = BMEM.get(a, tape[a] if a < len(tape) else 0)   # initial memory IS the tape image
            R[tape[pc + 1]] = v; pc += 3
        elif op == 0x09:                                 # storeb d, s — SYMBOLIC values are stored UNTRUNCATED:
            a = _concrete(reg(tape[pc + 1]), 'byte store to symbolic address')     # the observable is mod 256
            if any(0 <= a - b0 < 512 for (b0, t0, r0) in BMEM.get(SEGK, ())):
                raise Unsupported('byte store over a fill segment')
            if any(isinstance(w, int) and w <= a < w + 8 for w in MEM):            # and +/-/* respect mod-256
                raise Unsupported('byte access aliases a word slot')               # congruence, so every
            v = reg(tape[pc + 2])                                                  # observed byte stays exact.
            BMEM[a] = (v & 0xFF) if isinstance(v, int) else v
            pc += 3
        elif op == 0x0C:                                 # jmp a
            pc = imm8(pc + 1)
        elif op == 0x0D or op == 0x0E:                   # jz / jnz c, a
            c = reg(tape[pc + 1])
            if isinstance(c, tuple) and c and c[0] in ('blt', 'ble', 'beq', 'bne'):
                # a symbolic boolean guard — a direct comparison OR a STORED boolean flowing back into a
                # branch. jz exits (jumps) when c==0; jnz exits on fall-through. Continue = the other edge.
                if op == 0x0D:
                    exit_pc, cont_pc = imm8(pc + 2), pc + 10
                else:
                    exit_pc, cont_pc = pc + 10, imm8(pc + 2)
                kind = {'blt': 'lt', 'ble': 'le', 'beq': 'eq', 'bne': 'ne'}[c[0]]
                nxt = summarize(('cmp', kind, c[1], c[2]), cont_pc, exit_pc)
                if nxt is None:                          # not a summarizable loop: FORK on the branch.
                    if fork_depth >= 8:                  # cont_pc is the guard-TRUE edge, exit_pc the false
                        raise Unsupported('too many data-dependent branches')
                    tv = _exec(tape, backedges, cont_pc, dict(R), dict(MEM), dict(BMEM), sp, ncell, fork_depth + 1)
                    fv = _exec(tape, backedges, exit_pc, dict(R), dict(MEM), dict(BMEM), sp, ncell, fork_depth + 1)
                    return ('cond', c, _term(tv), _term(fv))
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
                    R[rz] = ('blt', x, y)   # sides stay RAW (ints keep summarize's slot matching)
                elif iop == 0x0F and fv == 1:              # jlt swapped: (y <= x)   counter y, bound x  (i<=n)
                    R[rz] = ('ble', y, x)
                else:                                      # jeq (==) / swapped (!=) — not a summarizable guard
                    R[rz] = ('beq' if fv == 0 else 'bne', x, y)
                pc = lj
            elif op == 0x0F:
                pc = imm8(pc + 3) if _s64(x) < _s64(y) else pc + 11
            else:
                pc = imm8(pc + 3) if x == y else pc + 11
        elif op == 0x11:                                 # read d — a fixed-index input var while the read
            cur = MEM.get(RDV, 0)                        # position is concrete; a stream element (k 7 pos)
            if isinstance(cur, int):                     # after a read-loop has made the position symbolic
                R[tape[pc + 1]] = ('v', cur); MEM[RDV] = cur + 1
                ncell[0] = max(ncell[0], cur + 1)
            else:
                R[tape[pc + 1]] = ('sv', cur); MEM[RDV] = _add(cur, 1)
            pc += 2
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
