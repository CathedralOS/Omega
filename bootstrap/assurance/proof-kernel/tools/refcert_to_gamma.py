#!/usr/bin/env python3
# refcert_to_gamma.py — translate a refinement-gate certificate (check.beta input syntax) into checker.gamma's
# (check PROOF GOAL) expression, for the refinement-cert diamond's THIRD leg. The cert's constructor terms
# (k CID args..) map to checker.gamma's CURRIED constructor encoding (Apply.. (Con CID) ..) — no gamma-side
# feature was needed; Con/Apply thread through pnorm/nateq/subt/freet already, and ffire leaves a
# non-constructor scrutinee STUCK, which is exactly what a refl certificate needs. The (fun FID ..) rule
# declarations are INLINED at each (f FID arg) site as (Fapp arg (Frule cidA bodyA) (Frule cidB bodyB)) —
# BINARY applications (f FID a b) ride an (Fbundle a b) scrutinee with (y 0) -> (Par 0). Applications use
# the TABLE-CARRYING (Fap tab fid arg) encoding: the whole rule table rides each application and rule
# bodies reference sibling functions via Fcall/Fcall2 (re-attaching the table), so MUTUALLY recursive
# families (binary-numeral badd/bmul) translate finitely where rule-inlining would diverge. Reads the cert on stdin, prints the
# gamma expression on stdout; exits 1 on anything outside the refl-cert shape.
import sys
import threading

sys.setrecursionlimit(400000)          # meaning certs carry deep unary spines (12345-deep numerals)
threading.stack_size(512 * 1024 * 1024)


def parse(src):
    toks = src.replace('(', ' ( ').replace(')', ' ) ').split()
    pos = [0]

    def rd():
        t = toks[pos[0]]; pos[0] += 1
        if t != '(':
            return int(t) if t.lstrip('-').isdigit() else t
        out = []
        while toks[pos[0]] != ')':
            out.append(rd())
        pos[0] += 1
        return out

    forms = []
    while pos[0] < len(toks):
        forms.append(rd())
    return forms


def term(t, funs):
    if t == 'z':
        return 'Ze'
    if t == 'nil':
        return 'Lnil'
    h = t[0]
    if h == 'cons':
        return '(Lcons %s %s)' % (term(t[1], funs), term(t[2], funs))
    if h == 'app':                                 # built-in list APPEND (distinct from proof application)
        return '(Lapp %s %s)' % (term(t[1], funs), term(t[2], funs))
    if h == 'len':
        return '(Llen %s)' % term(t[1], funs)
    if h == 's':
        return '(Su %s)' % term(t[1], funs)
    if h == 'p':
        return '(Pl %s %s)' % (term(t[1], funs), term(t[2], funs))
    if h == 'm':
        return '(Mu %s %s)' % (term(t[1], funs), term(t[2], funs))
    if h == 'v':
        return '(Iv %d)' % t[1]
    if h == 'rec':
        return '(Reccall %d)' % t[1]
    if h == 'k':                                   # (k CID a..) -> curried (Apply.. (Con CID) ..)
        g = '(Con %d)' % t[1]
        for a in t[2:]:
            g = '(Apply %s %s)' % (g, term(a, funs))
        return g
    if h == 'y':                                   # (y 0) -> the binary application's extra argument
        return '(Par %d)' % t[1]
    if h == 'f':                                   # (f FID a [b]) -> table-carrying (Fap tab FID arg):
        tab = funs['#tab']                         # the WHOLE rule table rides the application, so
        if len(t) == 4:                            # mutually recursive families terminate (bodies
            return '(Fap %s %d (Fbundle %s %s))' % (   # reference siblings via Fcall/Fcall2, which
                tab, t[1], term(t[2], funs), term(t[3], funs))     # re-attach the same table)
        return '(Fap %s %d %s)' % (tab, t[1], term(t[2], funs))
    raise SystemExit('untranslatable term head: %s' % h)


def body_term(t, funs):                            # a rule body: f-references become Fcall/Fcall2
    if isinstance(t, list) and t and t[0] == 'recx':   # (recx i E) -> (Recx i <body E>) accumulator rec
        return '(Recx %d %s)' % (t[1], body_term(t[2], funs))
    if isinstance(t, list) and t and t[0] == 'f':
        if len(t) == 4:
            return '(Fcall2 %d %s %s)' % (t[1], body_term(t[2], funs), body_term(t[3], funs))
        return '(Fcall %d %s)' % (t[1], body_term(t[2], funs))
    if isinstance(t, list) and t and t[0] == 'k':
        g = '(Con %d)' % t[1]
        for a in t[2:]:
            g = '(Apply %s %s)' % (g, body_term(a, funs))
        return g
    if isinstance(t, list) and t and t[0] == 's':
        return '(Su %s)' % body_term(t[1], funs)
    if isinstance(t, list) and t and t[0] == 'p':
        return '(Pl %s %s)' % (body_term(t[1], funs), body_term(t[2], funs))
    if isinstance(t, list) and t and t[0] == 'm':
        return '(Mu %s %s)' % (body_term(t[1], funs), body_term(t[2], funs))
    return term(t, funs)                           # v/y/rec/z literals share the term translation


def build_tab(funs):                               # (Tcons (Trule gid cid body) ...) over ALL rules
    tab = 'Tnil'
    for fid in sorted(funs, reverse=True):
        for cid, b in reversed(funs[fid]):
            tab = '(Tcons (Trule %d %d %s) %s)' % (fid, cid, body_term(b, funs), tab)
    return tab


def prop(g, funs):
    if g[0] == 'All':
        return '(All %s)' % prop(g[1], funs)
    if g[0] == 'Exists':
        return '(Exists %s)' % prop(g[1], funs)
    if g[0] == '=':
        return '(Eq %s %s)' % (term(g[1], funs), term(g[2], funs))
    if g[0] == '->':
        return '(Arrow %s %s)' % (prop(g[1], funs), prop(g[2], funs))
    if g[0] == '&':
        return '(And %s %s)' % (prop(g[1], funs), prop(g[2], funs))
    if g[0] == '+':
        return '(Or %s %s)' % (prop(g[1], funs), prop(g[2], funs))
    if g[0] == 'bot':
        return 'Bot'
    if g[0] == 'Rel':                              # Mem (777) / ProdIs (778) / Perm (779) live as Rel props
        return '(Rel %s %s %s)' % (g[1], term(g[2], funs), term(g[3], funs))
    if g[0] == 'Pred':
        return '(Pred %s %s)' % (g[1], term(g[2], funs))
    raise SystemExit('untranslatable prop head: %s' % g[0])


def proof(p, funs):
    if p[0] == 'gen':
        return '(Gen %s)' % proof(p[1], funs)
    if p[0] == 'refl':
        return '(Refl %s)' % term(p[1], funs)
    if p[0] == 'hyp':
        return '(Hyp %d)' % p[1]
    if p[0] == 'lam':                              # (lam P body) -> (Lam <prop> <proof>)
        return '(Lam %s %s)' % (prop(p[1], funs), proof(p[2], funs))
    if p[0] == 'app':
        return '(App %s %s)' % (proof(p[1], funs), proof(p[2], funs))
    if p[0] == 'inst':
        return '(Inst %s %s)' % (proof(p[1], funs), term(p[2], funs))
    if p[0] == 'eqelim':                           # (eqelim motive pfeq pfpa)
        return '(Eqelim %s %s %s)' % (prop(p[1], funs), proof(p[2], funs), proof(p[3], funs))
    if p[0] == 'natind':                           # (natind motive base step) -> Natind (built-in nat)
        return '(Natind %s %s %s)' % (prop(p[1], funs), proof(p[2], funs), proof(p[3], funs))
    if p[0] == 'rec':                              # (rec cidA cidB motive base step) -> Rec + two Mkspec
        sa, sb = funs['#data'][p[1]], funs['#data'][p[2]]
        return '(Rec (Mkspec %d %d %d %d) (Mkspec %d %d %d %d) %s %s %s)' % (
            p[1], sa[0], sa[1], sa[2], p[2], sb[0], sb[1], sb[2],
            prop(p[3], funs), proof(p[4], funs), proof(p[5], funs))
    if p[0] == 'prodrec':                          # (prodrec cid motive case) -> Prodrec + spec
        cid = p[1]; sp = funs['#data'][cid]        # the guard rides the spec CONSTRUCTOR: Mkprod only when cid
        ctor = 'Mkprod' if cid in funs['#prod'] else 'Mkspec'   # was (prod cid)-declared; else Mkspec -> gamma Bad
        return '(Prodrec (%s %d %d %d %d) %s %s)' % (
            ctor, cid, sp[0], sp[1], sp[2], prop(p[2], funs), proof(p[3], funs))
    # --- lemma citation: checker.gamma has no def/use, so INLINE the lemma's (closed) proof at each use ---
    if p[0] == 'use':
        return proof(funs['#defs'][int(p[1])], funs)
    # --- conjunction / disjunction / falsity / existential (checker.gamma nodes; arities match the cert) ---
    if p[0] == 'pair':   return '(Pair %s %s)' % (proof(p[1], funs), proof(p[2], funs))
    if p[0] == 'fst':    return '(Fst %s)' % proof(p[1], funs)
    if p[0] == 'snd':    return '(Snd %s)' % proof(p[1], funs)
    if p[0] == 'inl':    return '(Inl %s %s)' % (prop(p[1], funs), proof(p[2], funs))
    if p[0] == 'inr':    return '(Inr %s %s)' % (prop(p[1], funs), proof(p[2], funs))
    if p[0] == 'case':   return '(Case %s %s %s)' % (proof(p[1], funs), proof(p[2], funs), proof(p[3], funs))
    if p[0] == 'absurd': return '(Absurd %s %s)' % (prop(p[1], funs), proof(p[2], funs))
    if p[0] == 'disj':   return '(Disj %s)' % proof(p[1], funs)
    if p[0] == 'sinj':   return '(Sinj %s)' % proof(p[1], funs)
    if p[0] == 'wit':    return '(Wit %s %s %s)' % (prop(p[1], funs), term(p[2], funs), proof(p[3], funs))
    if p[0] == 'unpack': return '(Unpack %s %s)' % (proof(p[1], funs), proof(p[2], funs))
    if p[0] == 'listind':                          # built-in-list induction (nil / cons cases)
        return '(Listind %s %s %s)' % (prop(p[1], funs), proof(p[2], funs), proof(p[3], funs))
    # --- inductive relational predicates: Mem (777), ProdIs (778), Perm (779) intros + inversions ---
    if p[0] == 'memhead': return '(MemHead %s %s)' % (term(p[1], funs), term(p[2], funs))
    if p[0] == 'memtail': return '(MemTail %s %s)' % (term(p[1], funs), proof(p[2], funs))
    if p[0] == 'memcons': return '(MemCons %s)' % proof(p[1], funs)
    if p[0] == 'memnil':  return '(MemNil %s)' % proof(p[1], funs)
    if p[0] == 'pnil':    return '(Pnil)'
    if p[0] == 'pcons':   return '(Pcons %s %s)' % (term(p[1], funs), proof(p[2], funs))
    if p[0] == 'prodnilinv':  return '(Prodnilinv %s)' % proof(p[1], funs)
    if p[0] == 'prodconsinv': return '(Prodconsinv %s)' % proof(p[1], funs)
    if p[0] == 'permnil':  return '(Permnil)'
    if p[0] == 'permskip': return '(Permskip %s %s)' % (term(p[1], funs), proof(p[2], funs))
    if p[0] == 'permswap': return '(Permswap %s %s %s)' % (term(p[1], funs), term(p[2], funs), term(p[3], funs))
    if p[0] == 'permtrans':return '(Permtrans %s %s)' % (proof(p[1], funs), proof(p[2], funs))
    raise SystemExit('untranslatable proof head: %s' % p[0])


def main():
    forms = parse(sys.stdin.read())
    funs = {}                                      # fid -> [(cidA, bodyA), (cidB, bodyB)]
    specs = {}                                     # cid -> (arity, r0, r1)
    prods = set()                                  # cids declared (prod cid) — licensed for prodrec (Mkprod)
    defs = {}                                      # lemma id -> its (closed) proof, inlined at each (use N)
    body = []
    for f in forms:
        if isinstance(f, list) and f and f[0] == 'data':
            specs[f[1]] = (int(f[2]), int(f[3]), int(f[4]))   # cid -> (arity, r0, r1) for Rec's Mkspec
            continue
        if isinstance(f, list) and f and f[0] == 'prod':
            prods.add(f[1])                                   # (prod cid) — sole-constructor product marker
            continue
        if isinstance(f, list) and f and f[0] == 'fun':
            funs.setdefault(f[1], []).append((f[2], f[3]))
            continue
        if isinstance(f, list) and f and f[0] == 'def':       # (def N type proof) — a lemma; the type (f[2])
            defs[int(f[1])] = f[3]                             # is re-inferred when the proof is inlined at (use N)
            continue
        body.append(f)
    if len(body) != 2:
        raise SystemExit('expected exactly <goal> <proof> after declarations')
    funs['#tab'] = build_tab({k: v for k, v in funs.items() if k not in ('#tab', '#data', '#defs')})
    funs['#data'] = specs
    funs['#prod'] = prods
    funs['#defs'] = defs
    print('(check %s %s)' % (proof(body[1], funs), prop(body[0], funs)))


if __name__ == '__main__':
    rc = []                            # deep recursion needs real stack: run in a big-stack thread
    t = threading.Thread(target=lambda: rc.append(main()))
    t.start()
    t.join()
    sys.exit(rc[0] or 0 if rc else 1)
