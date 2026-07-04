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

FUEL = 200000


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
    return 'z' if k == 0 else '(s %s)' % nat(k - 1)


class V:                                   # a value: concrete int `n` + the TERM tree that computes it
    __slots__ = ('n', 't')

    def __init__(self, n, t=None):
        self.n = n
        self.t = t if t is not None else nat(n)


def main():
    forms = parse_all(sys.stdin.read())
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

    def ev(e, env):
        fuel[0] -= 1
        if fuel[0] <= 0:
            raise Out('fuel exhausted')
        if isinstance(e, int):
            return V(e)
        if isinstance(e, str):
            if e in env:
                return env[e]
            return V(0)                     # interp.beta's env_lookup returns 0 on a miss — mirror it
                                            # (omega2gamma emits at least one unbound reference in the wild)
        h = e[0]
        if h == '+':
            a, b = ev(e[1], env), ev(e[2], env)
            return V(a.n + b.n, '(p %s %s)' % (a.t, b.t))
        if h == '*':
            a, b = ev(e[1], env), ev(e[2], env)
            return V(a.n * b.n, '(m %s %s)' % (a.t, b.t))
        if h in ('-', '/', '%'):
            raise Out('operator %s outside the +/* fragment (the tv-encode user-fun route is the next slice)' % h)
        if h in ('<', '<=', '==', '!=', 'eq', 'lt', 'le', 'ne'):    # comparisons decided concretely
            a, b = ev(e[1], env), ev(e[2], env)
            r = {'<': a.n < b.n, '<=': a.n <= b.n, '==': a.n == b.n, '!=': a.n != b.n,
                 'eq': a.n == b.n, 'lt': a.n < b.n, 'le': a.n <= b.n, 'ne': a.n != b.n}[h]
            return V(1 if r else 0)
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
    exit_code = r.n & 0xFF
    good = nat(r.n)
    print('%d (= %s %s) (refl %s)' % (exit_code, r.t, good, good))
    print('(= %s (s %s)) (refl (s %s))' % (r.t, good, good))   # the negative control: off-by-one claim


if __name__ == '__main__':
    try:
        main()
    except Out as e:
        sys.stderr.write('outside fragment: %s\n' % e)
        sys.exit(2)
    except RecursionError:
        sys.stderr.write('outside fragment: recursion depth\n')
        sys.exit(2)
