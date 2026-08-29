#!/usr/bin/env python3
# check_ref.py — an INDEPENDENT reference proof checker written from the
# natural-deduction rules, not ported from check.beta. Reads "<goal> <proof>" on
# stdin and prints the same accept/reject interface as the authoritative checker.
#
# The Beta checker is lattice-lineage and authoritative. This Python program is
# one untrusted, auditable differential reference for its complete retained core:
# intuitionistic propositional logic
# (->, &, +, bot with intro+elim), PLUS first-order (All/Exists, de Bruijn), equality by conversion (refl +
# Peano/list/user-function normalization), and the FULL induction fragment (natind/listind/eqelim/disj/sinj).
# PLUS the inductive predicates Mem/ProdIs/Perm (Rel 777/778/779), generic structural induction over user
# datatypes (rec + con_case), and named lemmas (def/use). check_ref now realizes EVERY rule of check.beta.
# check-ref-diamond.sh fuzzes it against check.beta on random propositional/FO/equality/TV proofs and curated
# induction + predicate + lemma corpora, requiring identical accept/reject. UNTRUSTED and checked, like the other
# *_ref tools; a bug here (or in check.beta) surfaces as a disagreement — the trust anchor's one fully independent,
# auditable second implementation.
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

# ---- definitional equality: normalize (Peano + USER FUNCTIONS), then compare --------------------
# Terms:  z | (s t) | (p a b)=plus | (m a b)=times | (v i)=Ivar/pattern-var | (k cid arg..)=constructor
#       | (f fid arg [extra])=user-function application.  Declarations `(data cid ..)` / `(fun fid cid body)`
# populate FUNS. The `refl` proof + the conversion rule make `(= a b)` accept when a,b reduce to one form.
# This is exactly the machinery the translation-validation certificates use, so check_ref can validate them.
sys.setrecursionlimit(200000)
FUNS = {}                                              # (fid, cid) -> rule body ; from (fun fid cid body)
DATA = {}                                              # cid -> (arity, r0, r1) ; from (data cid arity r0 r1)
PRODUCTS = set()                                        # cids explicitly declared (prod cid) as the SOLE constructor
                                                       # of their type — the only cids `prodrec` may eliminate
LEMMAS = {}                                            # N -> verified prop ; from (def N type proof), cited by (use N)
DEFS_OK = True                                         # cleared if any (def ..) fails to verify -> whole cert rejects
FUEL = 2_000_000

def normalize(t, fuel=FUEL):
    if not isinstance(t, list) or fuel <= 0:
        return t                                       # z / atom / out of fuel (stuck)
    h = t[0]
    if h == 's':
        return ['s', normalize(t[1], fuel - 1)]
    if h == 'p':                                       # plus:  0+b=b ; (s a)+b = s(a+b)
        a = normalize(t[1], fuel - 1)
        if a == 'z':
            return normalize(t[2], fuel - 1)
        if isinstance(a, list) and a[0] == 's':
            return ['s', normalize(['p', a[1], t[2]], fuel - 1)]
        return ['p', a, normalize(t[2], fuel - 1)]     # stuck (open) — stays normal
    if h == 'm':                                       # times: 0*b=0 ; (s a)*b = b + a*b
        a = normalize(t[1], fuel - 1)
        if a == 'z':
            return 'z'
        if isinstance(a, list) and a[0] == 's':
            return normalize(['p', t[2], ['m', a[1], t[2]]], fuel - 1)
        return ['m', a, normalize(t[2], fuel - 1)]
    if h == 'cons':                                    # list cons: normalize head and tail
        return ['cons', normalize(t[1], fuel - 1), normalize(t[2], fuel - 1)]
    if h == 'app':                                     # append: nil++l=l ; (cons h t)++l = cons h (t++l)
        a = normalize(t[1], fuel - 1)
        if a == 'nil':
            return normalize(t[2], fuel - 1)
        if isinstance(a, list) and a[0] == 'cons':
            return ['cons', a[1], normalize(['app', a[2], t[2]], fuel - 1)]
        return ['app', a, normalize(t[2], fuel - 1)]   # stuck (open) — stays normal
    if h == 'len':                                     # length: len nil=z ; len (cons h t) = s (len t)
        a = normalize(t[1], fuel - 1)
        if a == 'nil':
            return 'z'
        if isinstance(a, list) and a[0] == 'cons':
            return ['s', normalize(['len', a[2]], fuel - 1)]
        return ['len', a]
    if h == 'k':                                       # constructor value: normalize its fields
        return ['k', t[1]] + [normalize(a, fuel - 1) for a in t[2:]]
    if h == 'f':                                        # user-function application (f fid scrut [extra])
        fid, scrut = t[1], t[2]
        extra = t[3] if len(t) > 3 else None
        r = reduce_fun(fid, scrut, extra, fuel - 1)
        if r is not None:
            return normalize(r, fuel - 1)
        return ['f', fid, normalize(scrut, fuel - 1)] + ([normalize(extra, fuel - 1)] if extra is not None else [])
    return t                                           # (v i) etc. — normal

def reduce_fun(fid, scrut, extra, fuel):               # one rewrite of (f fid scrut extra), or None if stuck
    # Constructor matching is weak-head: fields need not be normalized merely
    # to select the rule. This is observable only in work/fuel for pure terms,
    # and keeps framed constructor trees bounded like the authoritative checker.
    a = scrut if isinstance(scrut, list) and scrut and scrut[0] == 'k' else normalize(scrut, fuel)
    if isinstance(a, list) and a and a[0] == 'k':
        body = FUNS.get((fid, a[1]))
        if body is not None:
            return instantiate(body, fid, a[2:], extra)
    return None

def instantiate(t, fid, fields, extra):                # substitute a rule body (structural, no fuel)
    if not isinstance(t, list):
        return t
    h = t[0]
    if h == 'v':                                       # pattern var (v 0)/(v 1) -> scrutinee field
        i = int(t[1])
        return fields[i] if i < len(fields) else t
    if h == 'y':                                       # (y k) -> the extra argument
        return extra
    if h == 'rec':                                     # (rec i) -> recursive call on field i
        f = fields[int(t[1])]
        return ['f', fid, f] if extra is None else ['f', fid, f, extra]
    if h == 'recx':                                    # (recx i E) -> recursive call on field i with the
        f = fields[int(t[1])]                          # extra REPLACED by E (accumulator recursion; still
        return ['f', fid, f, instantiate(t[2], fid, fields, extra)]   # structurally decreasing on field i)
    if h == 'f':                                       # nested (f gid ...): keep gid, recurse args
        return ['f', t[1]] + [instantiate(a, fid, fields, extra) for a in t[2:]]
    if h == 'k':                                       # (k cid ...): keep cid, recurse args
        return ['k', t[1]] + [instantiate(a, fid, fields, extra) for a in t[2:]]
    if h == 's':
        return ['s', instantiate(t[1], fid, fields, extra)]
    if h in ('p', 'm'):
        return [h, instantiate(t[1], fid, fields, extra), instantiate(t[2], fid, fields, extra)]
    return t

def conv(a, b):                                        # definitional equality of terms
    return normalize(a) == normalize(b)

def prop_eq(p, q):                                     # proposition equality up to conversion (check.beta type_eq)
    if not isinstance(p, list) or not isinstance(q, list):
        return p == q                                  # bare atoms
    if p[0] != q[0]:
        return False
    h = p[0]
    if h == 'bot':
        return True
    if h == '=':
        return conv(p[1], q[1]) and conv(p[2], q[2])
    if h == 'Pred':
        return p[1] == q[1] and conv(p[2], q[2])
    if h == 'Rel':
        return p[1] == q[1] and conv(p[2], q[2]) and conv(p[3], q[3])
    if h in ('All', 'Exists'):
        return prop_eq(p[1], q[1])
    if h in ('->', '&', '+'):
        return prop_eq(p[1], q[1]) and prop_eq(p[2], q[2])
    return False

# ---- capture-avoiding de Bruijn machinery on individual terms/props ------------------------------
def shift_term(t, d, cut):
    if isinstance(t, list):
        if t[0] == 'v':
            i = int(t[1])
            return ['v', str(i + d)] if i >= cut else t
        if t[0] == 's':
            return ['s', shift_term(t[1], d, cut)]
        if t[0] in ('p', 'm', 'cons', 'app'):
            return [t[0], shift_term(t[1], d, cut), shift_term(t[2], d, cut)]
        if t[0] == 'len':
            return ['len', shift_term(t[1], d, cut)]
        if t[0] in ('f', 'k'):                         # user fun/constructor application: fid/cid + term args
            return [t[0], t[1]] + [shift_term(a, d, cut) for a in t[2:]]
    return t                                           # z / nil

def shift_prop(p, d, cut):
    if not isinstance(p, list):
        return p                                       # bare atom
    h = p[0]
    if h == 'bot':
        return p
    if h == '=':
        return ['=', shift_term(p[1], d, cut), shift_term(p[2], d, cut)]
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
        if t[0] in ('p', 'm', 'cons', 'app'):
            return [t[0], subst_term(t[1], s, depth), subst_term(t[2], s, depth)]
        if t[0] == 'len':
            return ['len', subst_term(t[1], s, depth)]
        if t[0] in ('f', 'k'):
            return [t[0], t[1]] + [subst_term(a, s, depth) for a in t[2:]]
    return t                                           # z / nil

def subst_prop(p, s, depth):
    if not isinstance(p, list):
        return p
    h = p[0]
    if h == 'bot':
        return p
    if h == '=':
        return ['=', subst_term(p[1], s, depth), subst_term(p[2], s, depth)]
    if h == 'Pred':
        return ['Pred', p[1], subst_term(p[2], s, depth)]
    if h == 'Rel':
        return ['Rel', p[1], subst_term(p[2], s, depth), subst_term(p[3], s, depth)]
    if h in ('All', 'Exists'):
        return [h, subst_prop(p[1], shift_term(s, 1, 0), depth + 1)]   # lift s across the binder
    return [h, subst_prop(p[1], s, depth), subst_prop(p[2], s, depth)]

def subst_term_keep(t, s, depth):                      # like subst_term but KEEPS the binder (no decrement)
    if isinstance(t, list):
        if t[0] == 'v':
            return s if int(t[1]) == depth else t
        if t[0] == 's':
            return ['s', subst_term_keep(t[1], s, depth)]
        if t[0] in ('p', 'm', 'cons', 'app'):
            return [t[0], subst_term_keep(t[1], s, depth), subst_term_keep(t[2], s, depth)]
        if t[0] == 'len':
            return ['len', subst_term_keep(t[1], s, depth)]
        if t[0] in ('f', 'k'):
            return [t[0], t[1]] + [subst_term_keep(a, s, depth) for a in t[2:]]
    return t

def subst_prop_keep(p, s, depth):                      # for induction's P(s n): substitute, keep the binder
    if not isinstance(p, list):
        return p
    h = p[0]
    if h == 'bot':
        return p
    if h == '=':
        return ['=', subst_term_keep(p[1], s, depth), subst_term_keep(p[2], s, depth)]
    if h == 'Pred':
        return ['Pred', p[1], subst_term_keep(p[2], s, depth)]
    if h == 'Rel':
        return ['Rel', p[1], subst_term_keep(p[2], s, depth), subst_term_keep(p[3], s, depth)]
    if h in ('All', 'Exists'):
        return [h, subst_prop_keep(p[1], shift_term(s, 1, 0), depth + 1)]
    return [h, subst_prop_keep(p[1], s, depth), subst_prop_keep(p[2], s, depth)]

def con_case(cid, motive):                             # expected case type for constructor `cid` under `motive`
    arity, r0, r1 = DATA[cid]                           # generic structural induction, mirroring check.beta con_case
    if arity == 0:
        return subst_prop(motive, ['k', cid], 0)                          # P(cid)
    if arity == 1:
        body = subst_prop_keep(motive, ['k', cid, ['v', '0']], 0)         # P(cid a0), a0 = Iv0
        if r0 == 1:
            body = ['->', motive, body]                                   # IH: P(a0) = motive
        return ['All', body]
    mp = shift_prop(motive, 1, 1)                                         # arity 2: two binders, lift params by one
    body = subst_prop_keep(mp, ['k', cid, ['v', '1'], ['v', '0']], 0)     # P(cid a1 a0), a1 = Iv1, a0 = Iv0
    if r1 == 1:
        body = ['->', mp, body]                                          # IH for a0 (inner arrow)
    if r0 == 1:
        body = ['->', subst_prop_keep(mp, ['v', '1'], 0), body]          # IH for a1 (outer arrow)
    return ['All', ['All', body]]

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
    if h == '=':                                       # an equation: its children are TERMS, not props
        return term_has(p[1], k) or term_has(p[2], k)
    return mentions_ivar(p[1], k) or mentions_ivar(p[2], k)

def term_has(t, k):
    if isinstance(t, list):
        if t[0] == 'v':
            return int(t[1]) == k
        if t[0] == 's':
            return term_has(t[1], k)
        if t[0] in ('p', 'm', 'cons', 'app'):
            return term_has(t[1], k) or term_has(t[2], k)
        if t[0] == 'len':
            return term_has(t[1], k)
        if t[0] in ('f', 'k'):
            return any(term_has(a, k) for a in t[2:])
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
        if isinstance(f, list) and f[0] == '->' and x is not None and prop_eq(f[1], x):
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
                and prop_eq(l[1], s[1]) and prop_eq(r[1], s[2]) and prop_eq(l[2], r[2])):
            return l[2]
        return None
    if h == 'absurd':
        p = infer(pf[2], ctx, idep)                    # (absurd C p): p:bot -> C
        return pf[1] if p == ['bot'] else None
    if h == 'refl':
        return ['=', pf[1], pf[1]]                      # (refl t) : (= t t)  — conversion does the rest
    if h == 'gen':                                     # (gen p): forall-intro over a fresh individual
        t = infer(pf[1], ctx, idep + 1)
        return None if t is None else ['All', t]
    if h == 'inst':                                    # (inst p t): forall-elim -> body[t/x]
        t = infer(pf[1], ctx, idep)
        return subst_prop(t[1], pf[2], 0) if isinstance(t, list) and t[0] == 'All' else None
    if h == 'wit':                                     # (wit body t p): exists-intro, p : body[t/x]
        body, term, p = pf[1], pf[2], pf[3]
        r = infer(p, ctx, idep)
        return ['Exists', body] if r is not None and prop_eq(r, subst_prop(body, term, 0)) else None
    if h == 'unpack':                                  # (unpack epf handler): exists-elim
        e = infer(pf[1], ctx, idep)
        hd = infer(pf[2], ctx, idep)
        if not (isinstance(e, list) and e[0] == 'Exists'):
            return None
        if not (isinstance(hd, list) and hd[0] == 'All'
                and isinstance(hd[1], list) and hd[1][0] == '->'):
            return None
        ant, C = hd[1][1], hd[1][2]
        if not prop_eq(ant, e[1]) or mentions_ivar(C, 0):   # C must not depend on the witness var
            return None
        return subst_prop(C, 'z', 0)                   # drop the binder, lower outer vars
    if h == 'natind':                                  # (natind motive base step): Peano induction
        motive, base, step = pf[1], pf[2], pf[3]
        tb = infer(base, ctx, idep)
        if tb is None or not prop_eq(tb, subst_prop(motive, 'z', 0)):   # base : P(0)
            return None
        ts = infer(step, ctx, idep)
        want = ['All', ['->', motive, subst_prop_keep(motive, ['s', ['v', '0']], 0)]]  # All n. P(n)->P(s n)
        if ts is None or not prop_eq(ts, want):
            return None
        return ['All', motive]                         # forall n. P(n)
    if h == 'listind':                                 # (listind motive base step): list induction
        motive, base, step = pf[1], pf[2], pf[3]
        tb = infer(base, ctx, idep)
        if tb is None or not prop_eq(tb, subst_prop(motive, 'nil', 0)):   # base : P(nil)
            return None
        ts = infer(step, ctx, idep)
        t2 = shift_prop(motive, 1, 1)                                     # motive' (for P(t))
        t3 = subst_prop_keep(t2, ['cons', ['v', '1'], ['v', '0']], 0)     # P(cons h t)
        want = ['All', ['All', ['->', t2, t3]]]        # All h. All t. (P(t) -> P(cons h t))
        if ts is None or not prop_eq(ts, want):
            return None
        return ['All', motive]                         # forall l. P(l)
    if h == 'eqelim':                                  # (eqelim motive pf_eq pf_pa): Leibniz / transport
        motive, pf_eq, pf_pa = pf[1], pf[2], pf[3]
        te = infer(pf_eq, ctx, idep)
        if not (isinstance(te, list) and te[0] == '='):
            return None
        tpa = infer(pf_pa, ctx, idep)
        if tpa is None or not prop_eq(tpa, subst_prop(motive, te[1], 0)):   # pf_pa : P(a)
            return None
        return subst_prop(motive, te[2], 0)            # P(b)
    if h == 'disj':                                    # (disj pf): from (0 = s t) or (s t = 0), falsity
        te = infer(pf[1], ctx, idep)
        if not (isinstance(te, list) and te[0] == '='):
            return None
        a, b = normalize(te[1]), normalize(te[2])
        az = (a == 'z'); asuc = isinstance(a, list) and a[0] == 's'
        bz = (b == 'z'); bsuc = isinstance(b, list) and b[0] == 's'
        return ['bot'] if (az and bsuc) or (asuc and bz) else None
    if h == 'sinj':                                    # (sinj pf): from (s a = s b), (a = b)
        te = infer(pf[1], ctx, idep)
        if not (isinstance(te, list) and te[0] == '='):
            return None
        a, b = normalize(te[1]), normalize(te[2])
        if isinstance(a, list) and a[0] == 's' and isinstance(b, list) and b[0] == 's':
            return ['=', a[1], b[1]]
        return None
    # ---- inductive predicates: Mem (Rel 777), ProdIs (Rel 778), Perm (Rel 779). Each intro rule mirrors
    # check.beta exactly; the inversions are sound because the only closed sources of these Rels are the intros.
    def as_rel(pf_sub, rid):                            # infer pf_sub, require it prove (Rel rid a b); return the prop
        r = infer(pf_sub, ctx, idep)
        return r if isinstance(r, list) and r[0] == 'Rel' and r[1] == rid else None
    if h == 'memhead':                                 # (memhead x t): Mem(x, cons(x,t))
        x, t = pf[1], pf[2]
        return ['Rel', '777', x, ['cons', x, t]]
    if h == 'memtail':                                 # (memtail h pf): from Mem(x,t) infer Mem(x, cons(h,t))
        r = as_rel(pf[2], '777')
        return ['Rel', '777', r[2], ['cons', pf[1], r[3]]] if r else None
    if h == 'memcons':                                 # (memcons pf): from Mem(x, cons(h,t)) infer (x=h) + Mem(x,t)
        r = as_rel(pf[1], '777')
        if not r:
            return None
        L = normalize(r[3])
        if not (isinstance(L, list) and L[0] == 'cons'):
            return None
        return ['+', ['=', r[2], L[1]], ['Rel', '777', r[2], L[2]]]
    if h == 'memnil':                                  # (memnil pf): from Mem(x, nil) infer falsity
        r = as_rel(pf[1], '777')
        return ['bot'] if r and normalize(r[3]) == 'nil' else None
    if h == 'pnil':                                    # (pnil): ProdIs(nil, 1)
        return ['Rel', '778', 'nil', ['s', 'z']]
    if h == 'pcons':                                   # (pcons h pf): from ProdIs(t,m) infer ProdIs(cons h t, h*m)
        r = as_rel(pf[2], '778')
        return ['Rel', '778', ['cons', pf[1], r[2]], ['m', pf[1], r[3]]] if r else None
    if h == 'prodnilinv':                              # (prodnilinv pf): from ProdIs(nil, n) infer n = 1
        r = as_rel(pf[1], '778')
        return ['=', r[3], ['s', 'z']] if r and normalize(r[2]) == 'nil' else None
    if h == 'prodconsinv':                             # (prodconsinv pf): from ProdIs(cons h t, n) infer ∃m. n=h*m & ProdIs(t,m)
        r = as_rel(pf[1], '778')
        if not r:
            return None
        L = normalize(r[2])
        if not (isinstance(L, list) and L[0] == 'cons'):
            return None
        n1, h1, t1s = shift_term(r[3], 1, 0), shift_term(L[1], 1, 0), shift_term(L[2], 1, 0)
        return ['Exists', ['&', ['=', n1, ['m', h1, ['v', '0']]], ['Rel', '778', t1s, ['v', '0']]]]
    if h == 'permnil':                                 # (permnil): Perm(nil, nil)
        return ['Rel', '779', 'nil', 'nil']
    if h == 'permskip':                                # (permskip x pf): from Perm(a,b) infer Perm(cons x a, cons x b)
        r = as_rel(pf[2], '779')
        return ['Rel', '779', ['cons', pf[1], r[2]], ['cons', pf[1], r[3]]] if r else None
    if h == 'permswap':                                # (permswap x y r): Perm(cons x (cons y r), cons y (cons x r))
        x, y, rest = pf[1], pf[2], pf[3]
        return ['Rel', '779', ['cons', x, ['cons', y, rest]], ['cons', y, ['cons', x, rest]]]
    if h == 'permtrans':                               # (permtrans pf1 pf2): from Perm(a,b) & Perm(b,c) infer Perm(a,c)
        r1 = as_rel(pf[1], '779')
        r2 = as_rel(pf[2], '779')
        if not (r1 and r2 and conv(r1[3], r2[2])):     # shared middle term matches up to conversion
            return None
        return ['Rel', '779', r1[2], r2[3]]
    if h == 'use':                                     # (use N): cite a previously verified named lemma
        return LEMMAS.get(pf[1])
    if h == 'rec':                                     # (rec cidA cidB motive caseA caseB): generic structural induction
        cidA, cidB, motive, caseA, caseB = pf[1], pf[2], pf[3], pf[4], pf[5]
        tA = infer(caseA, ctx, idep)
        if tA is None or not prop_eq(tA, con_case(cidA, motive)):
            return None
        tB = infer(caseB, ctx, idep)
        if tB is None or not prop_eq(tB, con_case(cidB, motive)):
            return None
        return ['All', motive]                         # forall x. P(x)
    if h == 'prodrec':                                 # (prodrec cid motive case): SINGLE-constructor product
        cid, motive, case = pf[1], pf[2], pf[3]         # elimination — from P holding on the type's sole
        if cid not in PRODUCTS:                         # SOUNDNESS GUARD: prodrec proves forall x.P from ONE case,
            return None                                 # so it is sound ONLY on a type with ONE constructor. Unlike
                                                       # `rec` (structurally locked to 2 cases, hence used only on
                                                       # 2-constructor types), prodrec's single case would UNSOUNDLY
                                                       # prove forall over a SUM type if aimed at one of its
                                                       # constructors. So cid must be explicitly declared a product
                                                       # via (prod cid) — an auditable, opt-in assertion that cid is
                                                       # its type's sole constructor (the same trust basis as rec's
                                                       # author-supplied constructor set, but made explicit).
        tC = infer(case, ctx, idep)                     # From P holding on that sole constructor, conclude forall x.P(x).
        if tC is None or not prop_eq(tC, con_case(cid, motive)):
            return None
        return ['All', motive]
    return None

def register(forms):
    """Populate FUNS (rewrite rules), DATA (constructor shapes), and LEMMAS (named (def N type proof), each
    VERIFIED against its stated type in source order before it is citable). Sets DEFS_OK=False if any def fails
    to verify — the whole cert then rejects, matching check.beta. Returns the non-declaration forms (goal, proof)."""
    global DEFS_OK
    FUNS.clear(); DATA.clear(); LEMMAS.clear(); PRODUCTS.clear(); DEFS_OK = True
    rest = []
    theory_frozen = False
    for f in forms:
        if isinstance(f, list) and f and f[0] == 'fun':
            if theory_frozen or not (0 <= int(f[1]) < 768) or not (0 <= int(f[2]) < 64):
                DEFS_OK = False; break
            if (f[1], f[2]) in FUNS:
                DEFS_OK = False; break
            FUNS[(f[1], f[2])] = f[3]
        elif isinstance(f, list) and f and f[0] == 'data':
            arity, r0, r1 = int(f[2]), int(f[3]), int(f[4])
            if theory_frozen or not (0 <= int(f[1]) < 64) or f[1] in DATA:
                DEFS_OK = False; break
            if arity not in (0, 1, 2) or r0 not in (0, 1) or r1 not in (0, 1):
                DEFS_OK = False; break
            if (arity == 0 and (r0 or r1)) or (arity == 1 and r1):
                DEFS_OK = False; break
            DATA[f[1]] = (int(f[2]), int(f[3]), int(f[4]))
        elif isinstance(f, list) and f and f[0] == 'prod':
            if theory_frozen or not (0 <= int(f[1]) < 64) or f[1] in PRODUCTS:
                DEFS_OK = False; break
            PRODUCTS.add(f[1])                          # (prod cid) — author asserts cid is its type's SOLE
                                                       # constructor, licensing prodrec's one-case elimination
        elif isinstance(f, list) and f and f[0] == 'def':
            theory_frozen = True
            N, typ, proof = f[1], f[2], f[3]
            if not (0 <= int(N) < 32768) or N in LEMMAS:
                DEFS_OK = False; break
            r = infer(proof, [], 0)
            if r is None or not prop_eq(r, typ):
                DEFS_OK = False; break
            LEMMAS[N] = typ
        else:
            rest.append(f)
    return rest

def main():
    forms = register(parse_all(sys.stdin.read()))
    if not DEFS_OK:                                     # a named-lemma proof failed its stated type
        print('reject'); return
    if len(forms) != 2:
        print('reject'); return
    goal, proof = forms                                # a cert is <decls> <goal> <proof(refl ..)>
    r = infer(proof, [], 0)
    print('accept' if r is not None and prop_eq(r, goal) else 'reject')

if __name__ == '__main__':                             # importable (check-ref-fuzz.py reuses parse_all/infer)
    main()
