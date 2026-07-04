#!/usr/bin/env python3
# gamma2claim.py — UNTRUSTED encoder for meaning-route translation validation at the SUMMIT rung.
#
# Reads an omega2gamma-translated gamma program (defs + a final expression, fully CLOSED — Omega samples
# take no runtime input) on stdin and abstract-executes it, building the program's meaning as an UNFOLDED
# kernel arithmetic term: every `+` in the computation becomes a `(p A B)` node over unary numerals, so the
# claim
#       (= <meaning term> <unary exit>)   (refl <unary exit>)
# is accepted by delta/check.beta only if the kernel's own CONVERSION re-computes the entire arithmetic of
# the sample and reaches the same exit. Control (if / match arms, call targets) is decided by the encoder —
# the same trust shape as tv-encode.py's unrolled loops: a bad decision mis-states the meaning and the claim
# simply fails against the independently-run interpreter exit. Scope (slice 1): the `+`/`*` fragment —
# literals, let, def calls (recursion bounded by fuel), if, match on Pair/bare-constructor values. Samples
# using `-` are outside the fragment and reported as such (exit 2).
#
# stdout line 1: `<computed exit> <claim cert>`; line 2: the off-by-one NEGATIVE-control cert the kernel
# must reject. The gate cross-checks the exit against both the interpreter run and the documented intent.
import sys
sys.setrecursionlimit(200000)          # deep dispatch chains in translated samples

FUEL = 500000
MAXV = 20000                           # unary-numeral wall: a larger intermediate would exhaust the checker

# tv-encode.py's kernel-side user-nat machinery, verbatim (uadd/usub/umul/ueq/ult + fueled udiv/umod):
# engaged only when the sample uses - / %, so the +/* samples keep their kernel-native p/m forms.
USER_PRELUDE = (
    "(data 2 0 0 0) (data 3 1 1 0) "
    "(fun 20 2 (k 2)) (fun 20 3 (v 0)) "
    "(fun 21 2 (y 0)) (fun 21 3 (k 3 (rec 0))) "
    "(fun 22 2 (y 0)) (fun 22 3 (f 20 (rec 0))) "
    "(fun 23 2 (k 2)) (fun 23 3 (f 21 (y 0) (rec 0))) "
    "(fun 24 2 (k 3 (k 2))) (fun 24 3 (k 2)) "
    "(fun 25 2 (f 24 (y 0))) (fun 25 3 (f 26 (y 0) (v 0))) "
    "(fun 26 2 (k 2)) (fun 26 3 (f 25 (v 0) (y 0))) "
    "(fun 27 2 (k 2)) (fun 27 3 (k 3 (k 2))) "
    "(fun 28 2 (f 27 (y 0))) (fun 28 3 (f 29 (y 0) (v 0))) "
    "(fun 29 2 (k 2)) (fun 29 3 (f 28 (y 0) (v 0)))"
    " (data 4 2 0 0) "
    "(fun 42 4 (v 0)) (fun 43 4 (v 1))"
    " (fun 46 2 (k 2)) (fun 46 3 (f 47 (k 3 (v 0)) (k 4 (k 3 (v 0)) (y 0)))) "
    "(fun 47 2 (k 2)) (fun 47 3 (f 48 (f 28 (f 42 (y 0)) (f 43 (y 0))) (k 4 (v 0) (y 0)))) "
    "(fun 48 3 (k 2)) "
    "(fun 48 2 (k 3 (f 47 (f 42 (y 0)) (k 4 (f 22 (f 43 (f 43 (y 0))) (f 42 (f 43 (y 0)))) (f 43 (f 43 (y 0))))))) "
    "(fun 49 2 (k 2)) (fun 49 3 (f 50 (k 3 (v 0)) (k 4 (k 3 (v 0)) (y 0)))) "
    "(fun 50 2 (f 42 (y 0))) (fun 50 3 (f 51 (f 28 (f 42 (y 0)) (f 43 (y 0))) (k 4 (v 0) (y 0)))) "
    "(fun 51 3 (f 42 (f 43 (y 0)))) "
    "(fun 51 2 (f 50 (f 42 (y 0)) (k 4 (f 22 (f 43 (f 43 (y 0))) (f 42 (f 43 (y 0)))) (f 43 (f 43 (y 0))))))"
)


class Out(Exception):
    pass


def parse_all(src):
    toks = src.replace('(', ' ( ').replace(')', ' ) ').split()
    pos = [0]

    def rd():
        t = toks[pos[0]]; pos[0] += 1
        if t != '(':
            return int(t) if t.lstrip('-').isdigit() else t
        o = []
        while toks[pos[0]] != ')':
            o.append(rd())
        pos[0] += 1
        return o

    forms = []
    while pos[0] < len(toks):
        forms.append(rd())
    return forms


def nat(k):
    if k < 0:
        raise Out('negative value reached the term encoder')
    if k > MAXV:
        raise Out('intermediate %d exceeds the unary wall' % k)
    return 'z' if k == 0 else '(s %s)' % nat(k - 1)


def unat(k):                           # user-nat literal (k 3 (k 3 ... (k 2)))
    if k < 0:
        raise Out('negative value reached the term encoder')
    if k > MAXV:
        raise Out('intermediate %d exceeds the unary wall' % k)
    return '(k 2)' if k == 0 else '(k 3 %s)' % unat(k - 1)


class V:                                   # a value: concrete int `n` + the TERM tree(s) that compute it.
    __slots__ = ('n', 't', 'nt')           # zpair mode: n is the ℤ value, (t, nt) the (pos, neg) components

    def __init__(self, n, t, nt=None):
        self.n = n
        self.t = t
        self.nt = nt


class Under(Exception):                    # an underflowing subtraction: retry the sample in zpair mode
    pass


def main():
    src = sys.stdin.read()
    try:
        run(src, zpair=False)
    except Under:
        run(src, zpair=True)               # an underflow: re-encode with ℤ difference-pair values


def run(src, zpair):
    forms = parse_all(src)
    # USER mode when -,/,% appear: values ride user nats and ops become kernel user-fun applications.
    # ZPAIR mode (an underflowing subtraction was hit): every value is a (pos, neg) pair of user nats —
    # componentwise uadd for +, swapped for -, cross terms for * — and the claim P = uadd(exit, N) makes the
    # kernel verify pos - neg = exit in ℤ, with no negative ever materializing (the refinement pillar's
    # difference-pair move, replayed kernel-side).
    user = zpair or any(('(%s ' % op) in src for op in ('-', '/', '%'))
    lit = unat if user else nat
    Z0 = '(k 2)'
    defs = {}
    top = None
    for f in forms:
        if isinstance(f, list) and f and f[0] == 'def':
            defs[f[1]] = (f[2], f[3])
        else:
            top = f
    if top is None:
        raise Out('no top-level expression')
    fuel = [FUEL]
    vcs = []                               # SAFETY OBLIGATIONS: one kernel-checked claim per / and % site —
                                           # iszero(divisor) reduces to 0, i.e. the kernel re-computes the
                                           # divisor and confirms the division cannot trap (omega-rs's
                                           # obligations.rs concept, discharged by the lattice's own anchor)

    def ev(e, env):
        fuel[0] -= 1
        if fuel[0] <= 0:
            raise Out('fuel exhausted')
        if isinstance(e, int):
            return V(e, lit(e), Z0 if zpair else None)
        if isinstance(e, str):
            if e in env:
                return env[e]
            return V(0, lit(0), Z0 if zpair else None)   # interp.beta's env_lookup: 0 on a miss — mirror it
                                            # (omega2gamma emits at least one unbound reference in the wild)
        h = e[0]
        if h == '+':
            a, b = ev(e[1], env), ev(e[2], env)
            if abs(a.n) + abs(b.n) > MAXV:
                raise Out('intermediate exceeds the unary wall')
            if zpair:
                return V(a.n + b.n, '(f 21 %s %s)' % (a.t, b.t), '(f 21 %s %s)' % (a.nt, b.nt))
            return V(a.n + b.n, ('(f 21 %s %s)' if user else '(p %s %s)') % (a.t, b.t))
        if h == '*':
            a, b = ev(e[1], env), ev(e[2], env)
            if abs(a.n * b.n) > MAXV:
                raise Out('intermediate exceeds the unary wall')
            if zpair:                       # (p1-n1)(p2-n2) = (p1p2+n1n2) - (p1n2+n1p2)
                return V(a.n * b.n,
                         '(f 21 (f 23 %s %s) (f 23 %s %s))' % (a.t, b.t, a.nt, b.nt),
                         '(f 21 (f 23 %s %s) (f 23 %s %s))' % (a.t, b.nt, a.nt, b.t))
            return V(a.n * b.n, ('(f 23 %s %s)' if user else '(m %s %s)') % (a.t, b.t))
        if h == '-':
            a, b = ev(e[1], env), ev(e[2], env)
            if zpair:                       # (p1-n1) - (p2-n2) = (p1+n2) - (n1+p2)
                return V(a.n - b.n, '(f 21 %s %s)' % (a.t, b.nt), '(f 21 %s %s)' % (a.nt, b.t))
            if a.n < b.n:
                raise Under()               # retry the whole sample with ℤ difference-pair values
            return V(a.n - b.n, '(f 22 %s %s)' % (b.t, a.t))   # usub(b, a) = a - b
        if h == '/':
            a, b = ev(e[1], env), ev(e[2], env)
            if zpair:
                raise Out('division over difference pairs: later')
            if b.n == 0:
                raise Out('division by zero')
            if a.n // b.n > 800:
                raise Out('quotient exceeds the reduction wall')
            vcs.append('(= (f 24 %s) (k 2)) (refl (k 2))' % b.t)   # div-by-zero VC: iszero(divisor) = 0
            return V(a.n // b.n, '(f 46 %s %s)' % (a.t, b.t))
        if h == '%':
            a, b = ev(e[1], env), ev(e[2], env)
            if zpair:
                raise Out('mod over difference pairs: later')
            if b.n == 0:
                raise Out('mod by zero')
            if a.n // b.n > 800:
                raise Out('quotient exceeds the reduction wall')
            vcs.append('(= (f 24 %s) (k 2)) (refl (k 2))' % b.t)   # mod-by-zero VC: iszero(divisor) = 0
            return V(a.n % b.n, '(f 49 %s %s)' % (a.t, b.t))
        if h in ('<', '<=', '==', '!=', 'eq', 'lt', 'le', 'ne'):    # comparisons decided concretely
            a, b = ev(e[1], env), ev(e[2], env)
            r = {'<': a.n < b.n, '<=': a.n <= b.n, '==': a.n == b.n, '!=': a.n != b.n,
                 'eq': a.n == b.n, 'lt': a.n < b.n, 'le': a.n <= b.n, 'ne': a.n != b.n}[h]
            return V(1 if r else 0, lit(1 if r else 0), Z0 if zpair else None)
        if h == 'let':
            env2 = dict(env)
            env2[e[1]] = ev(e[2], env)
            return ev(e[3], env2)
        if h == 'if':
            c = ev(e[1], env)
            return ev(e[2] if c.n != 0 else e[3], env)
        if h == 'match':
            sub = ev_ctor(e[1], env)
            for arm in e[2:]:
                pat, body = arm[0], arm[1]
                bound = match(pat, sub)
                if bound is not None:
                    env2 = dict(env)
                    env2.update(bound)
                    return ev(body, env2)
            raise Out('no match arm fired')
        if isinstance(h, str) and h in defs:
            params, body = defs[h]
            args = [ev_ctor(x, env) for x in e[1:]]
            if len(args) != len(params):
                raise Out('arity mismatch calling %s' % h)
            return ev(body, dict(zip(params, args)))
        if isinstance(h, str) and h[0].isupper():
            return ev_ctor(e, env)
        raise Out('form %s outside the fragment' % h)

    def ev_ctor(e, env):                   # values may be constructor applications (Pair etc.)
        if isinstance(e, list) and e and isinstance(e[0], str) and e[0][0].isupper():
            return (e[0],) + tuple(ev_ctor(x, env) for x in e[1:])
        return ev(e, env)

    def match(pat, val):
        if isinstance(pat, str):
            if pat[0].isupper():
                return {} if val == pat or (isinstance(val, tuple) and val[0] == pat and len(val) == 1) else None
            return {pat: val}
        if isinstance(pat, list):
            if not (isinstance(val, tuple) and val[0] == pat[0] and len(val) == len(pat)):
                return None
            bound = {}
            for p2, v2 in zip(pat[1:], val[1:]):
                b2 = match(p2, v2)
                if b2 is None:
                    return None
                bound.update(b2)
            return bound
        return None

    r = ev(top, {})
    if not isinstance(r, V):
        raise Out('top-level value is not a number')
    if zpair:                              # claim P = uadd(exit, N): verifies pos - neg = exit in ℤ
        if not (0 <= r.n <= 255):
            raise Out('final ℤ value %d not a plain exit byte' % r.n)
        rhs = '(f 21 %s %s)' % (unat(r.n), r.nt)
        bad = '(f 21 %s %s)' % (unat(r.n + 1), r.nt)
        print('%d %s (= %s %s) (refl %s)' % (r.n, USER_PRELUDE, r.t, rhs, rhs))
        print('%s (= %s %s) (refl %s)' % (USER_PRELUDE, r.t, bad, bad))
        return
    exit_code = r.n & 0xFF
    good = lit(r.n)
    pre = (USER_PRELUDE + ' ') if user else ''
    wrap = '(k 3 %s)' if user else '(s %s)'
    print('%d %s(= %s %s) (refl %s)' % (exit_code, pre, r.t, good, good))
    print('%s(= %s %s) (refl %s)' % (pre, r.t, wrap % good, wrap % good))   # off-by-one negative control
    for vc in dict.fromkeys(vcs):          # lines 3+: the division-safety obligations, each kernel-checked
        print('%s%s' % (pre, vc))


if __name__ == '__main__':
    try:
        main()
    except Out as e:
        sys.stderr.write('outside fragment: %s\n' % e)
        sys.exit(2)
    except RecursionError:
        sys.stderr.write('outside fragment: recursion depth\n')
        sys.exit(2)
