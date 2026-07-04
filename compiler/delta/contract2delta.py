#!/usr/bin/env python3
# contract2delta.py — translate an Omega CONTRACT machine (requires/ensures) into the delta proposition
# it obligates, in PROVER syntax, so prover.py can DISCHARGE it automatically and the trust anchor check
# the certificate. This is the source-contract half of proof-carrying Omega (omega-rs's obligations/
# contracts concept): `machine M(params) requires R ensures E {}` asserts `∀params. R... -> E...`.
#
# Reads samples/math_proofs/main.omg on stdin; prints `NAME<TAB>PROP` per machine for the fragment it
# covers (equalities and </<=/!= over + * and nat-literals), or `NAME<TAB>UNSUPPORTED: <reason>` (ranges
# `in a..=b`, `Bag(..)` permutations — future). Params are de Bruijn `(v i)` under the ∀-prefix; requires
# are antecedents; nat literals expand to s-numerals; + -> (p ..), * -> (m ..), < -> Lt, <= -> Le,
# != -> (-> (= ..) (bot)).  With --perturb, the conclusion's RHS is succ'd into a FALSE proposition.
import re
import sys

PERTURB = '--perturb' in sys.argv[1:]


def toks(s):
    return re.findall(r'==|<=|!=|[<()+*]|\d+nat|[A-Za-z_][A-Za-z0-9_]*|\d+', s)


def numeral(n):
    return 'z' if n == 0 else '(s %s)' % numeral(n - 1)


def expr(ts, pos, env):
    t, pos = product(ts, pos, env)
    while pos < len(ts) and ts[pos] == '+':
        r, pos = product(ts, pos + 1, env)
        t = '(p %s %s)' % (t, r)
    return t, pos


def product(ts, pos, env):
    t, pos = atom(ts, pos, env)
    while pos < len(ts) and ts[pos] == '*':
        r, pos = atom(ts, pos + 1, env)
        t = '(m %s %s)' % (t, r)
    return t, pos


def atom(ts, pos, env):
    t = ts[pos]
    if t == '(':
        e, pos = expr(ts, pos + 1, env)
        return e, pos + 1
    m = re.fullmatch(r'(\d+)nat|(\d+)', t)
    if m:
        return numeral(int(m.group(1) or m.group(2))), pos + 1
    if t in env:                                       # a param -> its de Bruijn index
        return '(v %d)' % env[t], pos + 1
    raise ValueError('atom %r' % t)


def clause(s, env):                                    # one requires/ensures line -> a prover prop
    ts = toks(s)
    for op, mk in (('==', lambda a, b: '(= %s %s)' % (a, b)),
                   ('<=', lambda a, b: '(Le %s %s)' % (a, b)),
                   ('<', lambda a, b: '(Lt %s %s)' % (a, b)),
                   ('!=', lambda a, b: '(-> (= %s %s) (bot))' % (a, b))):
        if op in ts:
            i = ts.index(op)
            try:
                lhs, p = expr(ts, 0, env)
                assert p == i
                rhs, p = expr(ts, i + 1, env)
                assert p == len(ts)
                return mk(lhs, rhs)
            except (ValueError, AssertionError, IndexError):
                return None
    return None


def _two(inner):                                       # split "A B" (two balanced terms) -> (A, B)
    terms, i, n = [], 0, len(inner)
    while i < n and len(terms) < 2:
        while i < n and inner[i] == ' ':
            i += 1
        j, depth = i, 0
        while j < n and (depth or inner[j] != ' '):
            if inner[j] == '(':
                depth += 1
            elif inner[j] == ')':
                depth -= 1
            j += 1
        terms.append(inner[i:j])
        i = j
    return terms[0], terms[1]


def falsify(prop):                                     # --perturb: turn the conclusion into a FALSE prop
    m = re.fullmatch(r'\((=|Lt|Le) (.*)\)', prop)
    if not m:
        return prop
    op = m.group(1)
    a, b = _two(m.group(2))
    if op == '=':
        return '(= %s (s %s))' % (a, b)                # a = s(b): off by one
    return '(%s (s %s) %s)' % (op, b, a)               # s(b) </<= a: false whenever a </<= b


def main():
    src = sys.stdin.read()
    for mm in re.finditer(r'machine\s+(\w+)\s*\(([^)]*)\)(.*?)\{', src, re.S):
        name, params_s, body = mm.group(1), mm.group(2), mm.group(3)
        params = [p.split(':')[0].strip() for p in params_s.split(',') if ':' in p]
        env = {p: len(params) - 1 - i for i, p in enumerate(params)}   # de Bruijn under the ∀-prefix
        reqs, enss, mode = [], [], None
        for ln in body.splitlines():
            w = ln.strip()
            if w == 'requires':
                mode = reqs
            elif w == 'ensures':
                mode = enss
            elif w and mode is not None:
                mode.append(w)
        if not enss:
            continue
        rp = [clause(r, env) for r in reqs]
        ep = [clause(e, env) for e in enss]
        if any(p is None for p in rp) or any(p is None for p in ep) or len(ep) != 1:
            bad = next((c for c, p in list(zip(reqs, rp)) + list(zip(enss, ep)) if p is None), 'multiple ensures')
            print('%s\tUNSUPPORTED: %s' % (name, bad))
            continue
        concl = falsify(ep[0]) if PERTURB else ep[0]
        prop = concl
        for r in reversed(rp):
            prop = '(-> %s %s)' % (r, prop)
        for _ in params:
            prop = '(All %s)' % prop
        print('%s\t%s' % (name, prop))


if __name__ == '__main__':
    main()
