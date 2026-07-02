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
#        | (gen p) | (inst p t) | (wit PROP t p) | (unpack p handler)       -- first-order
# Prop  := ATOM | (-> A B) | (& A B) | (+ A B) | (bot) | (All B) | (Exists B) | (Pred n t) | (Rel n t t)
# Term  := z | (s t) | (v i)                                                -- (v i) is a de Bruijn Ivar
# infer(proof, ctx, idep) returns the proposition proved, or None (ill-typed -> reject). `ctx` is the list
# of (prop, push_idep) pushed by enclosing `lam`s; `idep` is the individual-binder depth (All/Exists entered).
# `(hyp i)` is de Bruijn (0 = innermost lam); its stored prop is lifted by (idep - push_idep) on lookup, so
# individual vars stay correct across quantifier binders. All quantifier substitution is capture-avoiding.
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

# ---- capture-avoiding de Bruijn machinery on individual terms/props ------------------------------
def shift_term(t, d, cut):
    if isinstance(t, list):
        if t[0] == 'v':
            i = int(t[1])
            return ['v', str(i + d)] if i >= cut else t
        if t[0] == 's':
            return ['s', shift_term(t[1], d, cut)]
    return t                                           # z

def shift_prop(p, d, cut):
    if not isinstance(p, list):
        return p                                       # bare atom
    h = p[0]
    if h == 'bot':
        return p
    if h == 'Pred':
        return ['Pred', p[1], shift_term(p[2], d, cut)]
    if h == 'Rel':
        return ['Rel', p[1], shift_term(p[2], d, cut), shift_term(p[3], d, cut)]
    if h in ('All', 'Exists'):
        return [h, shift_prop(p[1], d, cut + 1)]
    return [h, shift_prop(p[1], d, cut), shift_prop(p[2], d, cut)]   # -> & +

def subst_term(t, s, depth):
    if isinstance(t, list):
        if t[0] == 'v':
            i = int(t[1])
            if i == depth:
                return s
            return ['v', str(i - 1)] if i > depth else t
        if t[0] == 's':
            return ['s', subst_term(t[1], s, depth)]
    return t                                           # z

def subst_prop(p, s, depth):
    if not isinstance(p, list):
        return p
    h = p[0]
    if h == 'bot':
        return p
    if h == 'Pred':
        return ['Pred', p[1], subst_term(p[2], s, depth)]
    if h == 'Rel':
        return ['Rel', p[1], subst_term(p[2], s, depth), subst_term(p[3], s, depth)]
    if h in ('All', 'Exists'):
        return [h, subst_prop(p[1], shift_term(s, 1, 0), depth + 1)]   # lift s across the binder
    return [h, subst_prop(p[1], s, depth), subst_prop(p[2], s, depth)]

def mentions_ivar(p, k):                               # does Ivar k occur free in p?
    if not isinstance(p, list):
        return False
    h = p[0]
    if h == 'bot':
        return False
    if h == 'Pred':
        return term_has(p[2], k)
    if h == 'Rel':
        return term_has(p[2], k) or term_has(p[3], k)
    if h in ('All', 'Exists'):
        return mentions_ivar(p[1], k + 1)
    return mentions_ivar(p[1], k) or mentions_ivar(p[2], k)

def term_has(t, k):
    if isinstance(t, list):
        if t[0] == 'v':
            return int(t[1]) == k
        if t[0] == 's':
            return term_has(t[1], k)
    return False

def infer(pf, ctx, idep=0):                            # ctx: list of (prop, push_idep)
    if not isinstance(pf, list) or not pf:
        return None
    h = pf[0]
    if h == 'hyp':
        i = int(pf[1])
        if not 0 <= i < len(ctx):
            return None
        prop, pidep = ctx[len(ctx) - 1 - i]
        return shift_prop(prop, idep - pidep, 0)       # lift to the current binder depth
    if h == 'lam':
        b = infer(pf[2], ctx + [(pf[1], idep)], idep)
        return None if b is None else ['->', pf[1], b]
    if h == 'app':
        f = infer(pf[1], ctx, idep); x = infer(pf[2], ctx, idep)
        if isinstance(f, list) and f[0] == '->' and x is not None and f[1] == x:
            return f[2]
        return None
    if h == 'pair':
        a = infer(pf[1], ctx, idep); b = infer(pf[2], ctx, idep)
        return ['&', a, b] if a is not None and b is not None else None
    if h == 'fst':
        p = infer(pf[1], ctx, idep)
        return p[1] if isinstance(p, list) and p[0] == '&' else None
    if h == 'snd':
        p = infer(pf[1], ctx, idep)
        return p[2] if isinstance(p, list) and p[0] == '&' else None
    if h == 'inl':
        a = infer(pf[2], ctx, idep)                    # (inl B p): p:A -> (+ A B)
        return ['+', a, pf[1]] if a is not None else None
    if h == 'inr':
        b = infer(pf[2], ctx, idep)                    # (inr A p): p:B -> (+ A B)
        return ['+', pf[1], b] if b is not None else None
    if h == 'case':
        s = infer(pf[1], ctx, idep); l = infer(pf[2], ctx, idep); r = infer(pf[3], ctx, idep)
        if (isinstance(s, list) and s[0] == '+' and isinstance(l, list) and l[0] == '->'
                and isinstance(r, list) and r[0] == '->'
                and l[1] == s[1] and r[1] == s[2] and l[2] == r[2]):
            return l[2]
        return None
    if h == 'absurd':
        p = infer(pf[2], ctx, idep)                    # (absurd C p): p:bot -> C
        return pf[1] if p == ['bot'] else None
    if h == 'gen':                                     # (gen p): forall-intro over a fresh individual
        t = infer(pf[1], ctx, idep + 1)
        return None if t is None else ['All', t]
    if h == 'inst':                                    # (inst p t): forall-elim -> body[t/x]
        t = infer(pf[1], ctx, idep)
        return subst_prop(t[1], pf[2], 0) if isinstance(t, list) and t[0] == 'All' else None
    if h == 'wit':                                     # (wit body t p): exists-intro, p : body[t/x]
        body, term, p = pf[1], pf[2], pf[3]
        return ['Exists', body] if infer(p, ctx, idep) == subst_prop(body, term, 0) else None
    if h == 'unpack':                                  # (unpack epf handler): exists-elim
        e = infer(pf[1], ctx, idep)
        hd = infer(pf[2], ctx, idep)
        if not (isinstance(e, list) and e[0] == 'Exists'):
            return None
        if not (isinstance(hd, list) and hd[0] == 'All'
                and isinstance(hd[1], list) and hd[1][0] == '->'):
            return None
        ant, C = hd[1][1], hd[1][2]
        if ant != e[1] or mentions_ivar(C, 0):         # C must not depend on the witness var
            return None
        return subst_prop(C, 'z', 0)                   # drop the binder, lower outer vars
    return None

def main():
    forms = parse_all(sys.stdin.read())
    goal, proof = forms[0], forms[1]
    print('accept' if infer(proof, [], 0) == goal else 'reject')

if __name__ == '__main__':                             # importable (check-ref-fuzz.py reuses parse_all/infer)
    main()
