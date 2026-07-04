#!/usr/bin/env python3
# contract2delta.py — translate an Omega CONTRACT machine (requires/ensures) into the delta proposition
# it obligates, so a kernel certificate can DISCHARGE it. This is the source-contract half of proof-
# carrying Omega (omega-rs's obligations/contracts concept): `machine M(params) requires R ensures E {}`
# asserts `∀params. R... -> E...`, a proof obligation. Reads samples/math_proofs/main.omg on stdin,
# prints one line per machine: `NAME<TAB>PROP` for the equality-arithmetic fragment it covers, or
# `NAME<TAB>UNSUPPORTED: <reason>` otherwise (ranges `in a..=b`, `<`/`<=`, `!=`, `Bag(..)` — future).
#
# Fragment: params are usize; ensures/requires are `EXPR == EXPR`; EXPR over + * , nat-literals
# (`3nat`, `1`), and params. The delta prop is elab syntax:  (all p1 .. (-> req1 .. (= lhs rhs))).
import re
import sys


def toks(s):
    return re.findall(r'==|[()+*]|\d+nat|[A-Za-z_][A-Za-z0-9_]*|\d+', s)


def expr(ts, pos):                                     # a flat + / * expression -> elab term (left assoc)
    t, pos = product(ts, pos)
    while pos < len(ts) and ts[pos] == '+':
        r, pos = product(ts, pos + 1)
        t = '(+ %s %s)' % (t, r)
    return t, pos


def product(ts, pos):
    t, pos = atom(ts, pos)
    while pos < len(ts) and ts[pos] == '*':
        r, pos = atom(ts, pos + 1)
        t = '(* %s %s)' % (t, r)
    return t, pos


def atom(ts, pos):
    t = ts[pos]
    if t == '(':
        e, pos = expr(ts, pos + 1)
        return e, pos + 1                              # skip ')'
    m = re.fullmatch(r'(\d+)nat|(\d+)', t)             # `3nat` or `3` -> numeral
    if m:
        return (m.group(1) or m.group(2)), pos + 1
    if re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', t):     # a param variable
        return t, pos + 1
    raise ValueError('atom %r' % t)


def equality(s):                                       # `E == E` -> (= lhs rhs), or None if not an equality
    ts = toks(s)
    if '==' not in ts:
        return None
    try:
        lhs, pos = expr(ts, 0)
        assert ts[pos] == '=='
        rhs, pos = expr(ts, pos + 1)
        assert pos == len(ts)
    except (ValueError, AssertionError, IndexError):
        return None
    return '(= %s %s)' % (lhs, rhs)


def main():
    perturb = '--perturb' in sys.argv[1:]
    src = sys.stdin.read()
    # split into `machine NAME(params) [requires ...] ensures ... { }`
    for m in re.finditer(r'machine\s+(\w+)\s*\(([^)]*)\)(.*?)\{', src, re.S):
        name, params_s, body = m.group(1), m.group(2), m.group(3)
        if name == 'main' or '::' in src[m.start():m.start() + 40].split('(')[0]:
            continue
        params = [p.split(':')[0].strip() for p in params_s.split(',') if ':' in p]
        # collect requires / ensures clause lines
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
        req_props = [equality(r) for r in reqs]
        ens_props = [equality(e) for e in enss]
        if any(p is None for p in req_props) or any(p is None for p in ens_props):
            bad = [c for c, p in list(zip(reqs, req_props)) + list(zip(enss, ens_props)) if p is None]
            print('%s\tUNSUPPORTED: non-equality clause %r' % (name, bad[0]))
            continue
        concl = ens_props[0] if len(ens_props) == 1 else None
        if concl is not None and perturb:              # off-by-one: succ the conclusion's RHS
            mm = re.fullmatch(r'\(= (.*) (\S+|\([^()]*(?:\([^()]*\)[^()]*)*\))\)', concl)
            concl = '(= %s (s %s))' % (mm.group(1), mm.group(2)) if mm else concl
        if concl is None:
            print('%s\tUNSUPPORTED: multiple ensures' % name)
            continue
        prop = concl
        for rp in reversed(req_props):                 # requires become antecedents
            prop = '(-> %s %s)' % (rp, prop)
        for p in reversed(params):                     # params become universals
            prop = '(all %s %s)' % (p, prop)
        print('%s\t%s' % (name, prop))


if __name__ == '__main__':
    main()
