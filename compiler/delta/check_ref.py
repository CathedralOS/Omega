#!/usr/bin/env python3
# check_ref.py — an INDEPENDENT reference proof checker for delta's PROPOSITIONAL fragment, written from the
# natural-deduction typing rules, NOT ported from check.beta / checker.gamma. Reads "<goal> <proof>" on
# stdin and prints 'accept' iff the proof proves the goal, else 'reject' (matching check.beta's interface).
#
# WHY THIS EXISTS — the trust anchor is the one lattice component whose two implementations (check.beta in
# Beta, checker.gamma in Gamma) are BOTH lattice-lineage (compiled by bc). Every other rung now has a truly
# independent, auditable reference — alpha_ref.py (VM), asm_ref.py (assembler), bc2.py/beta_interp.py (bc),
# gamma_ref.py (meaning). This is that reference for the checker's core: intuitionistic propositional logic
# (->, &, +, bot with intro+elim). check-ref-diamond.sh fuzzes it against check.beta on random propositional
# proofs, requiring identical accept/reject. UNTRUSTED and checked, like the other *_ref tools; a bug here
# (or in check.beta) surfaces as a disagreement. (Quantifiers/equality/conversion are later slices.)
#
# Proof := (hyp i) | (lam PROP p) | (app f x) | (pair a b) | (fst p) | (snd p)
#        | (inl PROP p) | (inr PROP p) | (case s l r) | (absurd PROP p)
# Prop  := ATOM (bare Uppercase ident) | (-> A B) | (& A B) | (+ A B) | (bot)
# infer(proof, context) returns the proposition the proof proves, or None (ill-typed -> reject). `context`
# is the list of hypotheses pushed by enclosing `lam`s; `(hyp i)` is de Bruijn (0 = innermost lam).
import sys

def tokens(s):
    return s.replace('(', ' ( ').replace(')', ' ) ').split()

def parse(ts, i):
    if ts[i] == '(':
        out = []; i += 1
        while ts[i] != ')':
            node, i = parse(ts, i); out.append(node)
        return out, i + 1
    return ts[i], i + 1

def parse_all(s):
    ts = tokens(s); i = 0; out = []
    while i < len(ts):
        node, i = parse(ts, i); out.append(node)
    return out

def infer(pf, ctx):
    if not isinstance(pf, list) or not pf:
        return None
    h = pf[0]
    if h == 'hyp':
        i = int(pf[1])
        return ctx[len(ctx) - 1 - i] if 0 <= i < len(ctx) else None
    if h == 'lam':
        b = infer(pf[2], ctx + [pf[1]])
        return None if b is None else ['->', pf[1], b]
    if h == 'app':
        f = infer(pf[1], ctx); x = infer(pf[2], ctx)
        if isinstance(f, list) and f[0] == '->' and x is not None and f[1] == x:
            return f[2]
        return None
    if h == 'pair':
        a = infer(pf[1], ctx); b = infer(pf[2], ctx)
        return ['&', a, b] if a is not None and b is not None else None
    if h == 'fst':
        p = infer(pf[1], ctx)
        return p[1] if isinstance(p, list) and p[0] == '&' else None
    if h == 'snd':
        p = infer(pf[1], ctx)
        return p[2] if isinstance(p, list) and p[0] == '&' else None
    if h == 'inl':
        a = infer(pf[2], ctx)                          # (inl B p): p:A -> (+ A B)
        return ['+', a, pf[1]] if a is not None else None
    if h == 'inr':
        b = infer(pf[2], ctx)                          # (inr A p): p:B -> (+ A B)
        return ['+', pf[1], b] if b is not None else None
    if h == 'case':
        s = infer(pf[1], ctx); l = infer(pf[2], ctx); r = infer(pf[3], ctx)
        if (isinstance(s, list) and s[0] == '+' and isinstance(l, list) and l[0] == '->'
                and isinstance(r, list) and r[0] == '->'
                and l[1] == s[1] and r[1] == s[2] and l[2] == r[2]):
            return l[2]
        return None
    if h == 'absurd':
        p = infer(pf[2], ctx)                          # (absurd C p): p:bot -> C
        return pf[1] if p == ['bot'] else None
    return None

def main():
    forms = parse_all(sys.stdin.read())
    goal, proof = forms[0], forms[1]
    print('accept' if infer(proof, []) == goal else 'reject')

if __name__ == '__main__':                             # importable (check-ref-fuzz.py reuses parse_all/infer)
    main()
