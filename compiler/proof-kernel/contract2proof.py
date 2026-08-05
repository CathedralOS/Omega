#!/usr/bin/env python3
# contract2proof.py — translate an Omega CONTRACT machine (requires/ensures) into the kernel proposition
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

BAGEQ = 90   # opaque binary-relation id for `Bag(a) == Bag(b)` (multiset equality). The prover/kernel treat
#              Rel as UNINTERPRETED, which is exactly right for a proof VIEW carried requires->ensures: the
#              identity contract `Bag(x)==Bag(y) |- Bag(x)==Bag(y)` is P->P, no multiset axioms needed.


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
    mb = re.fullmatch(r'\s*Bag\(\s*(\w+)\s*\)\s*==\s*Bag\(\s*(\w+)\s*\)\s*', s)
    if mb:                                              # `Bag(a) == Bag(b)` -> opaque relation Rel BAGEQ a b
        a, b = mb.group(1), mb.group(2)                 # (a proof view; see BAGEQ). Both sides must be params.
        if a in env and b in env:
            return '(Rel %d (v %d) (v %d))' % (BAGEQ, env[a], env[b])
        return None
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


def clauses(s, env):   # one requires/ensures line -> a LIST of props. `expr in lo..=hi` (an inclusive range,
    # Omega interval surface) splits into the two bounds [lo <= expr, expr <= hi]; anything else is a single
    # clause. Splitting an ensures range into two INDEPENDENT obligations (proven separately, each standalone
    # with its own rewrite budget) is what lets the prover discharge them -- a conjunctive `lo<=e & e<=hi`
    # goal sits on a rewrite-cap knife-edge that blows the node budget, but the two obligations alone don't.
    mi = re.fullmatch(r'\s*(.+?)\s+in\s+(\d+)\s*\.\.=\s*(\d+)\s*', s)
    if mi:
        e, lo, hi = mi.group(1).strip(), mi.group(2), mi.group(3)
        lop, hip = clause('%s <= %s' % (lo, e), env), clause('%s <= %s' % (e, hi), env)
        return None if lop is None or hip is None else [lop, hip]
    c = clause(s, env)
    return None if c is None else [c]


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
    # a disequality `A != B` is `(-> (= A B) (bot))`, TRUE under the requires (e.g. i<j); its false form is
    # the plain equality `(= A B)` (false whenever the requires forces A<B). Handle this before the =/Lt/Le
    # off-by-one succ, so the != negative control is not vacuous (it used to pass the true prop through).
    mneq = re.fullmatch(r'\(-> \((= .*)\) \(bot\)\)', prop)
    if mneq:
        return '(%s)' % mneq.group(1)
    mrel = re.fullmatch(r'\(Rel (\d+) (.*)\)', prop)   # an opaque relation `Rel n a b`: SWAP the two arguments.
    if mrel:                                           # `Rel n b a` is a DIFFERENT uninterpreted prop, NOT
        a, b = _two(mrel.group(2))                     # entailed by `Rel n a b` (the kernel knows no symmetry),
        if a != b:                                     # so the identity contract's proof must fail to fit it.
            return '(Rel %s %s %s)' % (mrel.group(1), b, a)
    m = re.fullmatch(r'\((=|Lt|Le) (.*)\)', prop)
    if not m:
        return prop
    op = m.group(1)
    a, b = _two(m.group(2))
    if op == '=':
        return '(= %s (s %s))' % (a, b)                # a = s(b): off by one
    return '(%s (s %s) %s)' % (op, b, a)               # s(b) </<= a: false whenever a </<= b


_MOD = re.compile(r'(\w+|\([^()]*\))\s*%\s*(\d+)')


def lift_modulo(reqs, enss, params):
    # Model `EXPR % K` (K a positive constant) as a fresh variable carrying the modulo operator's RANGE FACT
    # `fresh < K` (a nonnegative remainder is strictly below the divisor). The prover has no `%`, and the trust
    # core needs none: the ensures is discharged against the operator's postcondition -- exactly how omega-rs
    # bounds an operator result. Each distinct `EXPR % K` becomes one fresh param + one `fresh < K` antecedent,
    # substituted textually into every requires/ensures line so the normal clause machinery handles the rest.
    seen, extra_params, extra_reqs = {}, [], []

    def repl(line):
        def sub(m):
            key = (m.group(1).strip(), m.group(2))
            if key not in seen:
                fresh = '__mod%d' % len(seen)
                seen[key] = fresh
                extra_params.append(fresh)
                extra_reqs.append('%s < %s' % (fresh, m.group(2)))
            return seen[key]
        return _MOD.sub(sub, line)

    new_reqs = [repl(r) for r in reqs]      # run repl over BOTH lists (populating extra_reqs/params) BEFORE
    new_enss = [repl(e) for e in enss]      # concatenating -- else the range-fact antecedents get dropped
    return new_reqs + extra_reqs, new_enss, params + extra_params


def main():
    src = sys.stdin.read()
    for mm in re.finditer(r'machine\s+(\w+)\s*\(([^)]*)\)(.*?)\{', src, re.S):
        name, params_s, body = mm.group(1), mm.group(2), mm.group(3)
        params = [p.split(':')[0].strip() for p in params_s.split(',') if ':' in p]
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
        reqs, enss, params = lift_modulo(reqs, enss, params)   # `EXPR % K` -> fresh var + range-fact antecedent
        env = {p: len(params) - 1 - i for i, p in enumerate(params)}   # de Bruijn under the ∀-prefix
        rl = [clauses(r, env) for r in reqs]                # each line -> a LIST of props (a range -> 2 bounds)
        el = [clauses(e, env) for e in enss]
        if any(x is None for x in rl) or any(x is None for x in el):
            bad = next((c for c, x in list(zip(reqs, rl)) + list(zip(enss, el)) if x is None), '?')
            print('%s\tUNSUPPORTED: %s' % (name, bad))
            continue
        reqprops = [p for lst in rl for p in lst]           # flat antecedent list (requires ranges expanded)
        obligations = [p for lst in el for p in lst]        # each ensures bound is its OWN obligation
        multi = len(obligations) > 1
        for k, ob in enumerate(obligations):                # emit one line per obligation; unique name if >1 so
            concl = falsify(ob) if PERTURB else ob          # the gate proves/perturbs each independently
            prop = concl
            for r in reversed(reqprops):
                prop = '(-> %s %s)' % (r, prop)
            for _ in params:
                prop = '(All %s)' % prop
            oname = '%s~%d' % (name, k) if multi else name
            print('%s\t%s' % (oname, prop))


if __name__ == '__main__':
    main()
