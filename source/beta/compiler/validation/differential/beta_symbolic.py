#!/usr/bin/env python3
# beta_symbolic.py SOURCEFILE — SYMBOLIC evaluation of a Beta program: the source-side dual of
# alpha/alpha_symbolic.py. It tree-walks the shared parser's AST over a mix of CONCRETE integers and SYMBOLIC Peano
# terms — each read_byte() is a fresh input variable — and reports what function of its inputs the SOURCE
# denotes, as a closed-form expression. Where beta_interp.py RUNS a Beta program on concrete input, this asks
# "what does the source MEAN, for all inputs?" without picking any.
#
# This is an untrusted diagnostic component. `symbolic_differential.py`
# compares its generated term with the machine-side term, checks term equality
# in the rooted checker, and differentially pins both on a finite input set. It
# does not establish an exact connection to the written Beta semantics.
#
# Scope mirrors alpha_symbolic: the concrete-control, non-negative-arithmetic fragment. Control flow that
# branches on a SYMBOLIC value (input-dependent loop bound), symbolic subtraction, div/mod, or symbolic memory
# raises Unsupported. Concrete-bounded loops/recursion UNROLL. read_byte order fixes the input numbering.
import sys, os
sys.setrecursionlimit(400000)          # deep-nat traversals (buffer addresses render as s^k chains)

HERE = os.path.dirname(os.path.abspath(__file__))


def find_repo_root(start):
    current = start
    while True:
        if os.path.isfile(os.path.join(current, 'tools', 'lattice', 'paths.sh')):
            return current
        parent = os.path.dirname(current)
        if parent == current:
            raise RuntimeError('cannot find repository root from %s' % start)
        current = parent


REPO_ROOT = os.environ.get('OMEGA_REPO_ROOT') or find_repo_root(HERE)
BETA_REFERENCE = os.environ.get(
    'OMEGA_PATH_BETA_REFERENCE',
    os.path.join(REPO_ROOT, 'source', 'beta', 'reference'),
)
sys.path.insert(0, BETA_REFERENCE)
from beta_parser import lex, Parser

NAT_CAP = 1 << 20
MASK = (1 << 64) - 1

class Unsupported(Exception):
    pass

# ---- Peano terms, rendered to the same syntax alpha_symbolic / the prover use --------------------------
def nat(k):
    if k < 0 or k > NAT_CAP:
        raise Unsupported('constant %d too large for a Peano term' % k)
    t = ('z',)
    for _ in range(k):
        t = ('s', t)
    return t

# TRI_ID names the triangular-sum recurrence  g(0)=0, g(s k)=g(k)+k  (so g(n) = Σ_{0<=j<n} j). A loop that
# does `acc += i` per iteration over trip count t computes g(t); expressed as ('f', TRI_ID, t) it stays a
# closed FORM the checker accepts by refl on a symbolic input (the recurrence body is prepended as (fun ..)).
TRI_ID = 90
# ZZ_CID: a ℤ difference-pair value ('zz', pos, neg) means pos - neg (pos,neg Peano). Since the observable is
# mod 256 and 256 | 2^64, ℤ arithmetic mod 256 == alpha's mod-2^64 arithmetic mod 256, so this soundly models
# subtraction. Rendered as the constructor (k ZZ_CID pos neg); the checker accepts it by refl on symbolic pos/neg
# (the (data ZZ_CID 2 0 0) decl is prepended to the cert). Peano stays Peano — a value becomes 'zz' only once a
# subtraction touches it.
ZZ_CID = 5
# MN_CID: monus ('mn', a, b) = max(0, a - b), truncated subtraction over ℕ — the BRANCH-FREE trip count
# of a loop whose counter starts at a symbolic value: `i = a; while (i < n)` runs exactly n ∸ a times for ALL
# inputs (a > n gives 0 on the machine and in ℕ alike). Like zz, it is a plain binary constructor to the
# kernel — (data 6 2 0 0), certs by refl — with its MEANING carried by the two differentially-pinned engines.
MN_CID = 6
# SV_CID / SSUM_CID: the INPUT STREAM enters the term language. ('sv', t) = the t-th input byte, rendered
# (k 7 t); ('ssum', lo, hi) = Σ_{j=lo}^{hi-1} input[j], rendered (k 8 lo hi) — the closed form of a loop that
# does `acc += read_byte()` once per iteration. Fixed-index reads stay (v k), so all prior forms are
# unchanged; stream terms appear only when a read's index is symbolic (inside a summarized loop). Like
# zz/monus these are plain constructors to the kernel; their meaning lives in the pinned evaluators.
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
# DIV_CID / MOD_CID: integer division and remainder as OPAQUE binary constructors — ('div', a, b) = a / b,
# ('mod', a, b) = a % b (signed truncated, matching the machine). The SOURCE side must derive the SAME term the
# bytecode side does (alpha_symbolic), so the refinement equivalence is refl — no division axioms in the kernel.
DIV_CID = 14
MOD_CID = 15

def render(t):
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

def evaluate(t, env):                  # concrete value under {var_index: int} (ℤ; the gate observes it mod 256)
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
    return evaluate(t[1], env) * evaluate(t[2], env)

def _term(v):
    return nat(v) if isinstance(v, int) else v

# ---- ℤ difference-pair support (only engaged once a subtraction appears) -------------------------------
_ZERO = ('z',)
def _is_zz(v):  return isinstance(v, tuple) and v[0] == 'zz'
def _as_zz(v):  return (v[1], v[2]) if _is_zz(v) else (_term(v), _ZERO)       # lift a Peano value to pos - 0
def _padd(x, y):                       # Peano add of two terms, dropping an identity 0
    if x == _ZERO: return y
    if y == _ZERO: return x
    return ('p', x, y)
def _pmul(x, y):                       # Peano mul of two terms, dropping 0 and 1
    if x == _ZERO or y == _ZERO: return _ZERO
    if x == ONE: return y
    if y == ONE: return x
    return ('m', x, y)

def _add(a, b):
    if _is_zz(a) or _is_zz(b):
        (pa, na), (pb, nb) = _as_zz(a), _as_zz(b)
        return ('zz', _padd(pa, pb), _padd(na, nb))
    return (a + b) & MASK if isinstance(a, int) and isinstance(b, int) else ('p', _term(a), _term(b))

def _mul(a, b):
    if _is_zz(a) or _is_zz(b):          # (pa-na)(pb-nb) = (pa·pb + na·nb) - (pa·nb + na·pb)
        (pa, na), (pb, nb) = _as_zz(a), _as_zz(b)
        return ('zz', _padd(_pmul(pa, pb), _pmul(na, nb)), _padd(_pmul(pa, nb), _pmul(na, pb)))
    return (a * b) & MASK if isinstance(a, int) and isinstance(b, int) else ('m', _term(a), _term(b))

def _sub(a, b):                        # (pa-na) - (pb-nb) = (pa+nb) - (na+pb)
    if isinstance(a, int) and isinstance(b, int):
        if a >= b:
            return (a - b) & MASK      # non-underflowing literal difference — fold to a concrete nat, as +/* do
        (pa, na), (pb, nb) = _as_zz(a), _as_zz(b)   # underflow: a small ℤ pair, not a 2^64-1 wrap
        return ('zz', _padd(pa, nb), _padd(na, pb))
    (pa, na), (pb, nb) = _as_zz(a), _as_zz(b)
    return ('zz', _padd(pa, nb), _padd(na, pb))

def _s64(x):                           # a concrete 64-bit word as signed
    return x - (1 << 64) if x >= (1 << 63) else x

def _trunc_div(a, b):                  # signed division truncated toward zero (matches the machine)
    if b == 0:
        raise Unsupported('division by zero')   # the machine traps; not modelled
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q

def _divmod(op, a, b):                  # op in {'div','mod'}: fold two concretes (matching the machine), else an
    if isinstance(a, int) and isinstance(b, int):   # OPAQUE symbolic term identical to alpha_symbolic's
        q = _trunc_div(_s64(a), _s64(b))
        return (q if op == 'div' else _s64(a) - q * _s64(b)) & MASK
    if _is_zz(a) or _is_zz(b):
        raise Unsupported('div/mod on a signed (ℤ-pair) operand — not modelled yet')
    return (op, _term(a), _term(b))

def _concrete(v, why):
    if not isinstance(v, int):
        raise Unsupported(why)
    return v

ONE = ('s', ('z',))

# ---- linear-in-counter delta: decompose a per-iteration increment δ into a0 + a1·counter (a0,a1 invariant),
# so the loop's accumulator has closed form  init + a0·trip + a1·g(trip)  (g = the triangular sum). This unifies
# invariant deltas (a1=0), Σi (a0=0,a1=1), a·i (a0=0,a1=a), a+i (a0=a,a1=1), and combinations. ---------------
def _concnat(d):                       # int value of a concrete delta (int OR a Peano nat s^k z), else None
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

def _sum2(a, b):                       # a + b, dropping 0
    if a == 0: return b
    if b == 0: return a
    if isinstance(a, int) and isinstance(b, int): return a + b
    return ('p', _term(a), _term(b))

def _scale2(coef, x):                  # coef · x, dropping 0 and 1
    if x == 0 or coef == 0: return 0
    if isinstance(coef, int) and isinstance(x, int): return coef * x
    if x == 1: return coef
    if coef == 1: return x
    return ('m', _term(coef), _term(x))

def _lin_decompose(delta, counter, loop_vars):
    """delta over ('loopvar',*) placeholders -> (a0, a1) with delta == a0 + a1·(loopvar counter), a0/a1 free of
    ALL loop vars; None if delta is not linear in the counter (e.g. i·i, or depends on another loop var)."""
    ctr = ('loopvar', counter)
    def dec(t):
        if t == ctr:
            return (0, 1)
        if not _mentions_loopvar(t, loop_vars):
            return (t, 0)                                # a loop-invariant summand -> all of it is a0
        if isinstance(t, tuple):
            if t[0] == 'p':
                l, r = dec(t[1]), dec(t[2])
                return None if l is None or r is None else (_sum2(l[0], r[0]), _sum2(l[1], r[1]))
            if t[0] == 'm':
                inv, oth = ((t[1], t[2]) if not _mentions_loopvar(t[1], loop_vars) else
                            (t[2], t[1]) if not _mentions_loopvar(t[2], loop_vars) else (None, None))
                if inv is None:
                    return None                          # counter·counter (or counter·other-loopvar): non-linear
                d = dec(oth)
                return None if d is None else (_scale2(inv, d[0]), _scale2(inv, d[1]))
        return None
    return dec(delta)

def _series_closed(init, a0, a1, trip):
    """init + a0·trip + a1·g(trip), in a canonical form (both symbolic engines build this identically)."""
    def scaled(coef, base):
        if coef == 0: return None
        if coef == 1: return base
        return ('m', base, _term(coef))                  # base·coef  (matches the existing trip·delta ordering)
    r = _term(init)
    for p in (scaled(a0, trip), scaled(a1, ('f', TRI_ID, trip))):
        if p is not None:
            r = ('p', r, p)
    return r

def _down_series(p0, n0, a0p, a1p, a0n, a1n, trip):
    """Closed ℤ pair for a DOWN-counting loop (counter value n-k at iteration k, trip = n). A pair-delta with
    components a0p + a1p·i and a0n + a1n·i sums, after i ↦ n-k, to (a0x + a1x·n)·t - a1x·g(t) per component —
    the linear part joins the invariant coefficient and the triangular part FLIPS SIGN, crossing to the other
    component. Shared recipe in both engines so the forms stay byte-identical."""
    return ('zz', _series_closed(p0, _canon(_sum2(a0p, _scale2(a1p, trip))), _canon(a1n), trip),
                  _series_closed(n0, _canon(_sum2(a0n, _scale2(a1n, trip))), _canon(a1p), trip))

# ---- linear-loop analysis: read a per-iteration increment off ONE symbolic body execution ----------
def _mentions(t, ph):                  # does the placeholder `ph` occur in term `t`?
    if t == ph:
        return True
    if isinstance(t, tuple) and t[0] == 'f':
        return _mentions(t[2], ph)
    return isinstance(t, tuple) and t[0] in ('s', 'p', 'm', 'zz', 'mn', 'sv', 'ssum', 'blt', 'ble', 'beq', 'bne', 'cond') and any(_mentions(x, ph) for x in t[1:])

def _peel(expr, ph):
    """expr = an additive spine containing ph -> the spine with ph removed (the per-iteration delta,
    preserving the tree's shape so both engines build identical terms). Left-first when ambiguous —
    accumulator chains grow leftward. An UNROLLED inner concrete loop leaves exactly this shape:
    ((ph + a) + a) + a -> (a + a) + a. Mirror of alpha_symbolic._peel."""
    if expr == ph:
        return 0
    if isinstance(expr, tuple) and expr[0] == 'p':
        side = 1 if _mentions(expr[1], ph) else 2 if _mentions(expr[2], ph) else 0
        if side:
            d = _peel(expr[side], ph)
            other = expr[3 - side]
            if d is None:
                return None
            if d == 0:
                return other
            return ('p', d, other) if side == 1 else ('p', other, d)
    return None

def _lin_delta(expr, ph):
    """expr = ph + D or D + ph (D free of ph) -> D (the per-iteration increment); expr == ph -> 0; else None.
    Deeper additive spines (an unrolled inner concrete loop) peel via _peel. A zz pair ('zz', P, N) with
    P = ph + Dp and N free of ph is a SUBTRACTING accumulator: since +/- distribute componentwise over
    difference pairs, its pos/neg components follow independent additive recurrences — the delta is
    ('zz', Dp, N) and each component summarizes with the ordinary linear machinery."""
    if expr == ph:
        return 0
    if isinstance(expr, tuple) and expr[0] == 'p':
        a, b = expr[1], expr[2]
        if a == ph and not _mentions(b, ph):
            return b
        if b == ph and not _mentions(a, ph):
            return a
        d = _peel(expr, ph)
        if d is not None:
            return d
    if isinstance(expr, tuple) and expr[0] == 'zz' and not _mentions(expr[2], ph):
        dp = _lin_delta(expr[1], ph)
        if dp is not None:
            return ('zz', dp, expr[2])
    if isinstance(expr, tuple) and expr[0] == 'cond' and not _mentions(expr[1], ph):
        dT, dF = _lin_delta(expr[2], ph), _lin_delta(expr[3], ph)   # conditional post-value: per-branch deltas
        if dT is not None and dF is not None:
            return ('cond', expr[1], dT, dF)
    return None

def _mentions_loopvar(t, names):       # does term `t` mention any ('loopvar', v) with v in names?
    if isinstance(t, tuple):
        if t[0] == 'loopvar':
            return t[1] in names
        if t[0] == 'f':
            return _mentions_loopvar(t[2], names)
        if t[0] in ('s', 'p', 'm', 'zz', 'mn', 'sv', 'ssum', 'blt', 'ble', 'beq', 'bne', 'cond'):
            return any(_mentions_loopvar(x, names) for x in t[1:])
    return False

def _subst_loopvars(t, entry, invariant):   # ('loopvar', v) -> entry value for loop-INVARIANT v (raw, as
    if isinstance(t, tuple):                # alpha's _subst_slots splices raw MEM values); recurse elsewhere
        if t[0] == 'loopvar':
            return entry[t[1]] if t[1] in invariant else t
        if t[0] in ('s', 'sv'):
            return (t[0], _subst_loopvars(t[1], entry, invariant))
        if t[0] in ('p', 'm', 'zz', 'mn', 'ssum', 'blt', 'ble', 'beq', 'bne', 'cond'):
            return (t[0],) + tuple(_subst_loopvars(x, entry, invariant) for x in t[1:])
        if t[0] == 'f':
            return ('f', t[1], _subst_loopvars(t[2], entry, invariant))
    return t

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

def _expr_uses(e, names):              # does AST expression `e` reference any of the given variable names?
    if e[0] == 'var':
        return e[1] in names
    if e[0] == 'bin':
        return _expr_uses(e[2], names) or _expr_uses(e[3], names)
    if e[0] == 'call':
        return any(_expr_uses(a, names) for a in e[2])
    if e[0] == 'mem':
        return _expr_uses(e[2], names)
    return False

class SymInterp:
    def __init__(self, procs):
        self.procs = {p[1]: p for p in procs}
        self.bytemem = {}                  # concrete BYTE address -> value; SYMBOLIC values stored UNTRUNCATED
        self.rdpos = 0                     # the input-stream read position (int; a term after a read-loop)
        self.bytesegs = []                 # buffer SEGMENTS from summarized copy loops: (base, trip, rdbase) —
                                           # byte[base+j] = input[rdbase+j] for j < trip (a fill's closed form)
        self._fill = None                  # symbolic-address store events recorded during a region run
        self.n_inputs = 0                  # (the observable is mod 256; +/-/* respect mod-256 congruence)
        self.steps = 0

    def ev(self, e, env):
        self.steps += 1
        if self.steps > 2_000_000:
            raise Unsupported('step budget (a data-independent loop?)')
        k = e[0]
        if k == 'num':
            return e[1] & MASK
        if k == 'var':
            if e[1] not in env:
                raise Unsupported('read of a variable dropped by loop summarization (%s)' % e[1])
            return env[e[1]]
        if k == 'call':
            name = e[1]
            if name == 'read_byte':
                cur = self.rdpos
                if isinstance(cur, int):
                    self.rdpos = cur + 1
                    self.n_inputs = max(self.n_inputs, cur + 1)
                    return ('v', cur)                  # a fixed-index input variable (all prior forms)
                self.rdpos = _add(cur, 1)              # symbolic position: a STREAM element (k 7 idx)
                return ('sv', cur)
            if name == 'write_byte':
                return self.ev(e[2][0], env)                                # value flows through unchanged
            if name not in self.procs:
                raise Unsupported('unknown call %s' % name)
            return self.run(self.procs[name], [self.ev(a, env) for a in e[2]])
        if k == 'bin':
            op = e[1]; a = self.ev(e[2], env); b = self.ev(e[3], env)
            if op == '+':  return _add(a, b)
            if op == '*':  return _mul(a, b)
            if op == '-':  return _sub(a, b)
            if op == '/':  return _divmod('div', a, b)
            if op == '%':  return _divmod('mod', a, b)
            # comparisons: concrete operands decide to 0/1; a SYMBOLIC operand yields a boolean TERM
            # (blt/ble/beq/bne, >/>= normalized by bc's own operand swap) — the machine materializes the
            # same 0/1 the term evaluates to, so stored booleans and boolean arithmetic stay congruent.
            if not (isinstance(a, int) and isinstance(b, int)):
                op2, l, r = op, a, b
                if op2 in ('>', '>='):
                    op2, l, r = {'>': '<', '>=': '<='}[op2], r, l
                return ({'<': 'blt', '<=': 'ble', '==': 'beq', '!=': 'bne'}[op2], _term(l), _term(r))
            x = _concrete(a, 'compare on symbolic value'); y = _concrete(b, 'compare on symbolic value')
            from_signed = lambda z: z - (1 << 64) if z >= (1 << 63) else z
            sx, sy = from_signed(x), from_signed(y)
            return 1 if {'<': sx < sy, '>': sx > sy, '<=': sx <= sy, '>=': sx >= sy,
                         '==': x == y, '!=': x != y}[op] else 0
        if k == 'mem':
            if e[1] != 'byte':
                raise Unsupported('word memory not modelled yet')
            a = _concrete(self.ev(e[2], env), 'memory read at a symbolic address')
            for (b0, t0, r0) in self.bytesegs:          # a fill segment: byte[b0+j] = input[r0+j] for j < t0
                j = a - b0
                if 0 <= j < 512:
                    return ('cond', ('blt', j, t0), ('sv', r0 + j), self.bytemem.get(a, 0))
            return self.bytemem.get(a, 0)               # the interp's memory starts zeroed
        raise Unsupported('expression form %s' % k)

    def _summarize_loop(self, header_pc, body_label, cond, env, blocks, labels):
        """Recognize a linear counter loop  `state H { to B when (i<n) <exit> } state B { <lin updates> to H }`
        and replace it by the closed form of its accumulators, so a SYMBOLIC trip count is handled WITHOUT
        unrolling. The per-iteration increments are read off ONE exact symbolic body execution (each loop var
        set to a fresh placeholder); an accumulator with a loop-invariant delta d over trip count t becomes
        init + t*d. Returns True (env mutated to the post-loop state) iff the strict pattern matched; else the
        caller re-raises Unsupported. Non-unit strides, self-referential deltas (Σi), and non-zero counter
        starts are out of scope here — deliberately narrow so the closed form is exactly built-in +/*."""
        if body_label not in labels:
            return False
        self._sum_depth = getattr(self, '_sum_depth', 0) + 1
        try:
            return self._sum_depth <= 8 and self._summarize_loop_inner(header_pc, body_label, cond, env,
                                                                       blocks, labels)
        finally:
            self._sum_depth -= 1

    def _summarize_loop_inner(self, header_pc, body_label, cond, env, blocks, labels):
        entry = dict(env)
        ph_env = {v: ('loopvar', v) for v in env}       # EVERY var gets a placeholder (mirrors alpha, which
        rd_entry = self.rdpos                           # markers every frame slot); invariants subst back below
        rd_mark = ('loopvar', '#rd')                    # the read POSITION is a hidden loop var: in-body reads
        self.rdpos = rd_mark                            # come out as stream elements (sv #rd), stride-checked
        fill_save, self._fill = self._fill, []
        bf_save = getattr(self, '_body_forks', 0)
        try:
            out = self._run_region_once(labels[body_label], header_pc, ph_env, blocks, labels)
        except Unsupported:
            return False
        finally:
            rd_after, self.rdpos = self.rdpos, rd_entry
            fill_events, self._fill = self._fill, fill_save
            self._body_forks = bf_save
        reads = 0
        if rd_after != rd_mark:                         # the body consumed input: R reads per iteration
            rdd = _canon(_lin_delta(rd_after, rd_mark) or 0)
            if not isinstance(rdd, int) or rdd < 1 or isinstance(rd_entry, tuple):
                return False                            # a fixed per-iteration read count, from a fixed position
            reads = rdd
        seg_base = None
        if fill_events:                                 # a COPY loop: exactly one byte[base + ctr] = read_byte()
            if len(fill_events) != 1 or reads != 1:     # per iteration, base concrete, the value the iteration's
                return False                            # single stream element
            fa, fv = fill_events[0]
            if fv != ('sv', rd_mark):
                return False
            seg_base = None
            for cand in entry:                          # the counter is identified below; defer the base peel
                pass
            seg_addr = fa
        deltas = {}
        rewrite = set()                                 # REWRITE vars: fully overwritten each iteration (a
        for v in entry:                                 # temp t = a*i, …) — no additive delta exists. They
            d = _lin_delta(out[v], ('loopvar', v))      # are DROPPED post-loop (a later read refuses via ev);
            if d is None:                               # any OTHER delta reading their stale value refuses.
                rewrite.add(v)
                continue
            deltas[v] = d
        invariant = {v for v in entry if deltas.get(v) == 0}
        loop_vars = [v for v in entry if v in deltas and deltas[v] != 0]
        fresh = {}                                      # vars INTRODUCED by the body (e.g. an inner counter):
        for v in out:                                   # keep only values identical on every iteration; DROP
            if v not in entry:                          # the rest (unknowable post-loop — a later read of a
                fv = _subst_loopvars(out[v], entry, invariant)      # dropped var refuses via ev)
                if not _mentions_loopvar(fv, set(entry)) and not _mentions(fv, rd_mark) and not _has_stream(fv):
                    fresh[v] = fv
        if cond[0] == 'bin' and cond[1] in ('>', '>='):     # (a > b) ≡ (b < a): normalize to the < forms, the
            cond = ('bin', {'>': '<', '>=': '<='}[cond[1]], cond[3], cond[2])   # same swap bc does in codegen
        ne_mapped = False
        if cond[0] == 'bin' and cond[1] == '!=':
            ne_mapped = True
            # Over ℕ with a UNIT-stride counter, != is <: `i != n` from 0 by +1 hits n exactly (i < n), and
            # `i != 0` by -1 drains to 0 exactly (0 < i). The < / down branches below enforce precisely the
            # entry/stride conditions that make this exact-hit argument sound; any other != loop is refused
            # (e.g. stride 2 can SKIP the bound — the machine diverges — and _canon(delta) != 1 refuses it).
            if cond[2] == ('num', 0):                       # (0 != i)  ≡ (0 < i)
                cond = ('bin', '<', ('num', 0), cond[3])
            elif cond[3] == ('num', 0):                     # (i != 0)  ≡ (0 < i)
                cond = ('bin', '<', ('num', 0), cond[2])
            elif cond[2][0] == 'var' and cond[2][1] in loop_vars:   # (i != n) ≡ (i < n)  [entry 0, +1 below]
                cond = ('bin', '<', cond[2], cond[3])
            elif cond[3][0] == 'var' and cond[3][1] in loop_vars:   # (n != i) ≡ (i < n) — the counter is the
                cond = ('bin', '<', cond[3], cond[2])               # side that is a LOOP var
        if cond[0] != 'bin' or cond[1] not in ('<', '<='):
            return False
        down = False
        off = 0                                         # the up-counter's START value (0 keeps today's forms)
        if cond[2][0] == 'var':                         # UP-count: `i < bound` / `i <= bound`, i by +1
            counter = cond[2][1]
            if counter not in loop_vars or _canon(deltas[counter]) != 1:
                return False                            # counter: a unit-stride loop var
            off = _canon(entry[counter])
            if isinstance(off, tuple) and off[0] in ('zz', 'mn'):
                return False                            # a ℤ-pair / monus start value: later
            if ne_mapped and off != 0:
                return False                            # != needs the exact-hit argument, only sound from 0
            if _expr_uses(cond[3], loop_vars) or _expr_uses(cond[3], rewrite):
                return False                            # the bound must be loop-invariant
            try:
                bound = self.ev(cond[3], env)
            except Unsupported:
                return False
            hi = bound if cond[1] == '<' else _add(bound, 1)    # exclusive upper end: bound (<) or bound+1 (<=)
            # from 0: trip = hi (the existing forms). From a symbolic/nonzero start a: trip = hi ∸ a — MONUS,
            # the branch-free trip count (a > hi runs 0 times on the machine and 0 = hi ∸ a in ℕ alike).
            trip = hi if off == 0 else ('mn', _term(hi), _term(off))
        elif cond[1] == '<' and cond[2] == ('num', 0) and cond[3][0] == 'var':
            down = True                                 # DOWN-count: `0 < i`, i from I by -1 -> exactly I trips
            counter = cond[3][1]                        # (`0 <= i` with -1 never terminates: not recognized)
            if counter not in loop_vars or not _is_negone(deltas[counter]):
                return False
            trip = entry[counter]
            if isinstance(trip, tuple) and trip[0] == 'zz':
                return False                            # a ℤ-pair trip count: later
        else:
            return False
        closed = {}
        for v in loop_vars:                             # each δ = a0 + a1·counter -> init + a0·trip + a1·g(trip)
            d = deltas[v]
            if not (isinstance(d, tuple) and d[0] == 'zz') and (_has_stream(d)
                                                                or _mentions(d, ('loopvar', '#rd'))):
                rest, coef = _split_stream(d, ('loopvar', '#rd'))   # δ = rest + coef·read
                width = 1
                if coef is None:                        # not a single coefficiented read: try the WIDE shape —
                    rest, offs = _stream_offsets(d, ('loopvar', '#rd'))     # the acc consumes ALL R of the
                    if offs is None or sorted(offs) != list(range(reads)) or reads < 1:
                        return False                    # iteration's reads, each once -> Σ over base..base+R·t
                    coef, width = 1, reads
                elif reads != 1:
                    return False                        # one-of-many reads: a STRIDED sum — refused
                coef_s = _subst_loopvars(coef, entry, invariant) if isinstance(coef, tuple) else coef
                if (_has_stream(coef_s) or _has_zz(coef_s) or _mentions_loopvar(coef_s, loop_vars)
                        or _mentions(coef_s, ('loopvar', '#rd'))):
                    return False                        # the read's coefficient must be loop-invariant
                if rest == 0:
                    dec = (0, 0)
                else:
                    rest_s = _subst_loopvars(rest, entry, invariant)
                    if _has_stream(rest_s) or _has_zz(rest_s):
                        return False
                    dec = _lin_decompose(rest_s, counter, loop_vars)
                    if dec is None or (down and _canon(dec[1]) != 0):
                        return False
                if down and _canon(dec[1]) != 0:
                    return False                        # a counter-dependent rest under a down-counter
                rest_closed = _series_closed(entry[v], _canon(_sum2(dec[0], _scale2(dec[1], off))), _canon(dec[1]), trip)
                closed[v] = _read_sum(rest_closed, rd_entry, trip, coef_s, width)
                continue
            if isinstance(d, tuple) and d[0] == 'zz':   # subtracting accumulator: summarize pos/neg
                comps = []                              # independently — each component may carry its OWN
                for raw_comp in (d[1], d[2]):           # stream part (acc -= read puts the Σ on the NEG side)
                    rest, coef = _split_stream(raw_comp, ('loopvar', '#rd'))
                    if rest is None and coef is None:
                        return False                    # not linearly separable (read·read, …)
                    if coef is not None:
                        if reads != 1 or down:
                            return False                # stride-1 reads, up-counting only
                        coef = _subst_loopvars(coef, entry, invariant) if isinstance(coef, tuple) else coef
                        if (_has_stream(coef) or _has_zz(coef) or _mentions_loopvar(coef, loop_vars)
                                or _mentions_loopvar(coef, rewrite) or _mentions(coef, ('loopvar', '#rd'))):
                            return False                # the read's coefficient must be loop-invariant
                    rest_s = _subst_loopvars(rest, entry, invariant) if rest != 0 else 0
                    if rest_s != 0 and (_has_stream(rest_s) or _has_zz(rest_s)
                                        or _mentions_loopvar(rest_s, rewrite)
                                        or _mentions(rest_s, ('loopvar', '#rd'))):
                        return False
                    dec = (0, 0) if rest_s == 0 else _lin_decompose(rest_s, counter, loop_vars)
                    if dec is None:
                        return False
                    comps.append((dec, coef))
                (dp, pcoef), (dn, ncoef) = comps
                p0, n0 = _as_zz(entry[v])
                if down:                                # i ↦ n-k: linear parts fold into the invariant
                    closed[v] = _down_series(p0, n0, dp[0], dp[1], dn[0], dn[1], trip)
                else:
                    closed[v] = ('zz', _component_closed(p0, dp, pcoef, rd_entry, trip, off),
                                       _component_closed(n0, dn, ncoef, rd_entry, trip, off))
                continue
            sub = _subst_loopvars(d, entry, invariant)
            if _has_zz(sub):
                return False                            # an invariant zz value spliced into a plain delta
            if _mentions_loopvar(sub, rewrite):
                return False                            # the delta reads a rewrite var's stale value
            dec = _lin_decompose(sub, counter, loop_vars)
            if dec is None:
                return False                            # δ not linear in the counter (i·i, cross-loopvar): later
            if down and _canon(dec[1]) != 0:            # counter-dependent plain δ under a down-counter:
                p0, n0 = _as_zz(entry[v])               # the -a1·g(t) cross-term makes the result a ℤ pair
                closed[v] = _down_series(p0, n0, dec[0], dec[1], 0, 0, trip)
                continue
            closed[v] = _series_closed(entry[v], _canon(_sum2(dec[0], _scale2(dec[1], off))), _canon(dec[1]), trip)
        if fill_events:
            if down or off != 0:
                return False                            # copy loops: up-counting from 0 (slice 1)
            pb = _peel(seg_addr, ('loopvar', counter))
            base = _concnat(pb) if pb is not None else None
            if base is None:
                return False                            # store address must be exactly base + counter
            if any(b0 + 512 > base and base + 512 > b0 for (b0, t0, r0) in self.bytesegs) \
                    or any(base <= k2 < base + 512 for k2 in self.bytemem):
                return False                            # overlapping segments / prior writes: refused
            self.bytesegs.append((base, trip, rd_entry))
        env.update(closed)
        env.update(fresh)                               # body-introduced vars with iteration-independent values
        for v in rewrite:
            env.pop(v, None)                            # dropped: a post-loop read refuses via ev
        if reads:
            self.rdpos = _series_closed(rd_entry, 1, 0, trip)   # base + trip: symbolic -> later reads refuse
        return True

    def _run_region_once(self, start_idx, header_pc, env, blocks, labels, si=0):
        """Execute the loop-body REGION once on a placeholder env, following CONCRETE control only — an inner
        loop with a concrete bound unrolls right here, mirroring alpha's _run_body_once — until control jumps
        back to the outer header. Mutates and returns env. Symbolic branches (a nested SYMBOLIC loop — a
        later slice), returns, and call statements all refuse via Unsupported."""
        pc = start_idx
        steps = 0
        while True:
            jumped = False
            stmts = blocks[pc]
            i, si = si, 0
            while i < len(stmts):
                st = stmts[i]; i += 1
                steps += 1
                if steps > 200000:
                    raise Unsupported('loop body region too long to summarize')
                k = st[0]
                if k in ('let', 'assign'):
                    env[st[1]] = self.ev(st[2], env)
                elif k == 'callstmt':
                    self.ev(st[1], env)                 # result discarded; a read still advances the stream
                elif k == 'memset':
                    if st[1] != 'byte':
                        raise Unsupported('word memory not modelled yet')
                    a2 = self.ev(st[2], env)
                    v2 = self.ev(st[3], env)
                    if isinstance(a2, int):
                        raise Unsupported('concrete byte store inside a summarized loop body')
                    if self._fill is None:
                        raise Unsupported('symbolic-address byte store outside a fill-recording run')
                    self._fill.append((a2, v2))         # a FILL event: byte[base + ctr] = value, judged after
                elif k == 'goto':
                    take = st[2] is None
                    if st[2] is not None:
                        try:
                            take = _concrete(self.ev(st[2], env),
                                             'symbolic branch inside a summarized loop body') != 0
                        except Unsupported:             # an INNER loop with a symbolic bound summarizes
                            if self._summarize_loop(pc, st[1], st[2], env, blocks, labels):
                                take = False            # recursively; an IF-DIAMOND inside the body FORKS
                            else:                       # both paths to the header and merges pointwise
                                if getattr(self, '_body_forks', 0) >= 4:
                                    raise Unsupported('too many branches inside a summarized loop body')
                                self._body_forks = getattr(self, '_body_forks', 0) + 1
                                try:
                                    b = self._boolterm(st[2], env)
                                    rd, bm = self.rdpos, dict(self.bytemem)
                                    eT = self._run_region_once(labels[st[1]], header_pc, dict(env), blocks, labels)
                                    rdT, bmT = self.rdpos, self.bytemem
                                    self.rdpos, self.bytemem = rd, dict(bm)
                                    eF = self._run_region_once(pc, header_pc, dict(env), blocks, labels, si=i)
                                    if rdT != self.rdpos:
                                        raise Unsupported('paths consume different read counts')
                                    for kk in set(bmT) | set(self.bytemem):
                                        vT, vF = bmT.get(kk, bm.get(kk, 0)), self.bytemem.get(kk, bm.get(kk, 0))
                                        self.bytemem[kk] = vT if vT == vF else ('cond', b, vT, vF)
                                    merged = {}
                                    for kk in set(eT) | set(eF):
                                        vT, vF = eT.get(kk, env.get(kk, 0)), eF.get(kk, env.get(kk, 0))
                                        merged[kk] = vT if vT == vF else ('cond', b, vT, vF)
                                    env.clear(); env.update(merged)
                                    return env
                                finally:
                                    self._body_forks -= 1
                    if take:
                        if labels.get(st[1]) == header_pc:
                            return env                  # one full iteration: control returned to the header
                        pc = labels[st[1]]; jumped = True; break
                else:
                    raise Unsupported('statement %s inside a summarized loop body' % k)
            if not jumped:
                pc += 1
                if pc >= len(blocks):
                    raise Unsupported('loop body region fell off the program')

    def run(self, proc, argvals):
        _, name, params, body = proc
        env = {p: (argvals[i] if i < len(argvals) else 0) for i, p in enumerate(params)}
        blocks = [[]]; labels = {}
        for s in body:
            if s[0] == 'state':
                labels[s[1]] = len(blocks); blocks.append(s[2])
            else:
                blocks[0].append(s)
        return self._walk(0, 0, env, blocks, labels)

    def _boolterm(self, e, env):
        """The guard expression's value as a boolean TERM — a direct comparison or a STORED boolean
        (ev renders symbolic comparisons as blt/ble/beq/bne terms)."""
        v = self.ev(e, env)
        if isinstance(v, tuple) and v and v[0] in ('blt', 'ble', 'beq', 'bne'):
            return v
        raise Unsupported('branch on a non-boolean symbolic value')

    def _walk(self, pc, si, env, blocks, labels):
        """Execute blocks from (block pc, statement index si) to completion; return the RESULT value. A goto
        on a symbolic non-loop guard FORKS: both paths run to completion and the result is the conditional
        term (cond b then else) — no join detection needed. Path state (env, the read position, byte memory)
        is copied per path; a fork inside a callee is independent. Fork depth is capped."""
        while pc < len(blocks):
            stmts = blocks[pc]
            i, si = si, 0
            jumped = False
            while i < len(stmts):
                st = stmts[i]; i += 1
                self.steps += 1
                if self.steps > 2_000_000:
                    raise Unsupported('step budget (a data-independent loop?)')
                k = st[0]
                if k == 'let' or k == 'assign':
                    env[st[1]] = self.ev(st[2], env)
                elif k == 'return':
                    return self.ev(st[1], env)
                elif k == 'callstmt':
                    self.ev(st[1], env)
                elif k == 'memset':
                    if st[1] != 'byte':
                        raise Unsupported('word memory not modelled yet')
                    a = _concrete(self.ev(st[2], env), 'memory write at a symbolic address')
                    if any(0 <= a - b0 < 512 for (b0, t0, r0) in self.bytesegs):
                        raise Unsupported('byte store over a fill segment')
                    v = self.ev(st[3], env)
                    self.bytemem[a] = (v & 0xFF) if isinstance(v, int) else v
                elif k == 'goto':
                    take = st[2] is None
                    if st[2] is not None:
                        try:
                            take = _concrete(self.ev(st[2], env), 'branch on symbolic value') != 0
                        except Unsupported:            # symbolic guard: a data-dependent LOOP summarizes;
                            if self._summarize_loop(pc, st[1], st[2], env, blocks, labels):
                                take = False           # anything else FORKS into a conditional term
                            else:
                                self._forks = getattr(self, '_forks', 0) + 1
                                if self._forks > 8:
                                    raise Unsupported('too many data-dependent branches')
                                b = self._boolterm(st[2], env)
                                rd, bm = self.rdpos, dict(self.bytemem)
                                tv = self._walk(labels[st[1]], 0, dict(env), blocks, labels)
                                self.rdpos, self.bytemem = rd, bm
                                fv = self._walk(pc, i, dict(env), blocks, labels)
                                return ('cond', b, _term(tv), _term(fv))
                    if take:
                        pc = labels[st[1]]; jumped = True; break
                else:
                    raise Unsupported('statement form %s' % k)   # emit not modelled yet
            if not jumped:
                pc += 1
        return 0

def meaning(src_text):
    """-> (output_term, n_inputs): the closed-form meaning of `main`, over inputs (v 0)..(v n-1)."""
    procs = Parser(lex(src_text)).parse()
    si = SymInterp(procs)
    out = si.run(si.procs['main'], [])
    return _term(out), si.n_inputs

def main():
    with open(sys.argv[1]) as f:
        out, n = meaning(f.read())
    sys.stdout.write('%d %s\n' % (n, render(out)))

if __name__ == '__main__':
    main()
