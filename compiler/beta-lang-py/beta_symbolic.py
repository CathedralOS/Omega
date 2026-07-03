#!/usr/bin/env python3
# beta_symbolic.py SOURCEFILE — SYMBOLIC evaluation of a Beta program: the source-side dual of
# alpha/alpha_symbolic.py. It tree-walks bc2.py's AST over a mix of CONCRETE integers and SYMBOLIC Peano
# terms — each read_byte() is a fresh input variable — and reports what function of its inputs the SOURCE
# denotes, as a closed-form expression. Where beta_interp.py RUNS a Beta program on concrete input, this asks
# "what does the source MEAN, for all inputs?" without picking any.
#
# WHY — it turns instruction-level refinement into FULLY AUTOMATIC translation validation. alpha_symbolic
# derives what the COMPILED machine code computes; this derives what the SOURCE means; the refinement gate
# proves the two agree for ALL inputs (prover.py -> check.beta). Both derivations are independent and
# UNTRUSTED — this one is differentially pinned against beta_interp.py's concrete runs, exactly as the *_ref
# tools are — so a bc miscompile OR a bug in either symbolic evaluator surfaces as a rejected proof or a
# differential disagreement. No human writes the "claimed meaning" any more.
#
# Scope mirrors alpha_symbolic: the concrete-control, non-negative-arithmetic fragment. Control flow that
# branches on a SYMBOLIC value (input-dependent loop bound), symbolic subtraction, div/mod, or symbolic memory
# raises Unsupported. Concrete-bounded loops/recursion UNROLL. read_byte order fixes the input numbering.
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from bc2 import lex, Parser

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

def render(t):
    h = t[0]
    if h == 'z':  return 'z'
    if h == 's':  return '(s %s)' % render(t[1])
    if h == 'v':  return '(v %d)' % t[1]
    if h == 'f':  return '(f %d %s)' % (t[1], render(t[2]))
    if h == 'zz': return '(k %d %s %s)' % (ZZ_CID, render(t[1]), render(t[2]))
    return '(%s %s %s)' % (h, render(t[1]), render(t[2]))

def evaluate(t, env):                  # concrete value under {var_index: int} (ℤ; the gate observes it mod 256)
    h = t[0]
    if h == 'z':  return 0
    if h == 's':  return 1 + evaluate(t[1], env)
    if h == 'v':  return env[t[1]]
    if h == 'zz': return evaluate(t[1], env) - evaluate(t[2], env)
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
    return None

def _canon(x):                         # canonicalize a concrete coefficient to an int so 0/1 simplify identically
    c = _concnat(x)
    return c if c is not None else x

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

# ---- linear-loop analysis: read a per-iteration increment off ONE symbolic body execution ----------
def _mentions(t, ph):                  # does the placeholder `ph` occur in term `t`?
    if t == ph:
        return True
    return isinstance(t, tuple) and t[0] in ('s', 'p', 'm', 'zz') and any(_mentions(x, ph) for x in t[1:])

def _lin_delta(expr, ph):
    """expr = ph + D or D + ph (D free of ph) -> D (the per-iteration increment); expr == ph -> 0; else None.
    A zz pair ('zz', P, N) with P = ph + Dp and N free of ph is a SUBTRACTING accumulator: since +/- distribute
    componentwise over difference pairs, its pos/neg components follow independent additive recurrences — the
    delta is ('zz', Dp, N) and each component summarizes with the ordinary linear machinery."""
    if expr == ph:
        return 0
    if isinstance(expr, tuple) and expr[0] == 'p':
        a, b = expr[1], expr[2]
        if a == ph and not _mentions(b, ph):
            return b
        if b == ph and not _mentions(a, ph):
            return a
    if isinstance(expr, tuple) and expr[0] == 'zz' and not _mentions(expr[2], ph):
        dp = _lin_delta(expr[1], ph)
        if dp is not None:
            return ('zz', dp, expr[2])
    return None

def _mentions_loopvar(t, names):       # does term `t` mention any ('loopvar', v) with v in names?
    if isinstance(t, tuple):
        if t[0] == 'loopvar':
            return t[1] in names
        if t[0] in ('s', 'p', 'm', 'zz'):
            return any(_mentions_loopvar(x, names) for x in t[1:])
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
        self.n_inputs = 0
        self.steps = 0

    def ev(self, e, env):
        self.steps += 1
        if self.steps > 2_000_000:
            raise Unsupported('step budget (a data-independent loop?)')
        k = e[0]
        if k == 'num':
            return e[1] & MASK
        if k == 'var':
            return env[e[1]]
        if k == 'call':
            name = e[1]
            if name == 'read_byte':
                v = ('v', self.n_inputs); self.n_inputs += 1; return v      # a fresh input variable
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
            # comparisons: only decidable on concretes (that is what keeps control flow concrete)
            x = _concrete(a, 'compare on symbolic value'); y = _concrete(b, 'compare on symbolic value')
            from_signed = lambda z: z - (1 << 64) if z >= (1 << 63) else z
            sx, sy = from_signed(x), from_signed(y)
            return 1 if {'<': sx < sy, '>': sx > sy, '<=': sx <= sy, '>=': sx >= sy,
                         '==': x == y, '!=': x != y}[op] else 0
        raise Unsupported('expression form %s' % k)     # 'mem' (byte[]/word[]) not modelled yet

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
        body = blocks[labels[body_label]]
        if not body or body[-1][0] != 'goto' or body[-1][2] is not None or labels.get(body[-1][1]) != header_pc:
            return False                                # body must end with an unconditional jump back here
        updates = body[:-1]
        if any(s[0] not in ('let', 'assign') for s in updates):
            return False                                # body is straight-line assignments only
        loop_vars = [s[1] for s in updates]
        entry = {v: env.get(v, 0) for v in loop_vars}
        saved = dict(env)                               # read deltas off one body run with fresh placeholders
        for v in loop_vars:
            env[v] = ('loopvar', v)
        try:
            for s in updates:
                env[s[1]] = self.ev(s[2], env)
            deltas = {v: _lin_delta(env[v], ('loopvar', v)) for v in loop_vars}
        except Unsupported:
            deltas = None
        env.clear(); env.update(saved)                  # restore the entry state
        if deltas is None or any(d is None for d in deltas.values()):
            return False
        if cond[0] != 'bin' or cond[1] not in ('<', '<=') or cond[2][0] != 'var':
            return False
        counter = cond[2][1]
        if counter not in loop_vars or _canon(deltas[counter]) != 1 or entry[counter] != 0:
            return False                                # counter: a unit-stride loop var starting at 0
        if _expr_uses(cond[3], loop_vars):
            return False                                # the bound must be loop-invariant
        try:
            bound = self.ev(cond[3], env)
        except Unsupported:
            return False
        trip = bound if cond[1] == '<' else _add(bound, 1)      # iterations: 0..bound-1 (<) or 0..bound (<=)
        closed = {}
        for v in loop_vars:                             # each δ = a0 + a1·counter -> init + a0·trip + a1·g(trip)
            d = deltas[v]
            if isinstance(d, tuple) and d[0] == 'zz':   # subtracting accumulator: summarize pos/neg independently
                dp = _lin_decompose(d[1], counter, loop_vars)
                dn = _lin_decompose(d[2], counter, loop_vars)
                if dp is None or dn is None:
                    return False
                p0, n0 = _as_zz(entry[v])
                closed[v] = ('zz', _series_closed(p0, _canon(dp[0]), _canon(dp[1]), trip),
                                   _series_closed(n0, _canon(dn[0]), _canon(dn[1]), trip))
                continue
            dec = _lin_decompose(d, counter, loop_vars)
            if dec is None:
                return False                            # δ not linear in the counter (i·i, cross-loopvar): later
            closed[v] = _series_closed(entry[v], _canon(dec[0]), _canon(dec[1]), trip)
        env.update(closed)
        return True

    def run(self, proc, argvals):
        _, name, params, body = proc
        env = {p: (argvals[i] if i < len(argvals) else 0) for i, p in enumerate(params)}
        blocks = [[]]; labels = {}
        for s in body:
            if s[0] == 'state':
                labels[s[1]] = len(blocks); blocks.append(s[2])
            else:
                blocks[0].append(s)
        pc = 0
        while pc < len(blocks):
            jumped = False
            for st in blocks[pc]:
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
                elif k == 'goto':
                    take = st[2] is None
                    if st[2] is not None:
                        try:
                            take = _concrete(self.ev(st[2], env), 'branch on symbolic value') != 0
                        except Unsupported:            # symbolic guard: try to summarize a data-dependent loop
                            if self._summarize_loop(pc, st[1], st[2], env, blocks, labels):
                                take = False           # loop replaced by its closed form; fall to the exit
                            else:
                                raise
                    if take:
                        pc = labels[st[1]]; jumped = True; break
                else:
                    raise Unsupported('statement form %s' % k)   # memset/emit not modelled yet
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
