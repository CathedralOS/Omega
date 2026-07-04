#!/usr/bin/env python3
# refcert_to_gamma.py — translate a refinement-gate certificate (check.beta input syntax) into checker.gamma's
# (check PROOF GOAL) expression, for the refinement-cert diamond's THIRD leg. The cert's constructor terms
# (k CID args..) map to checker.gamma's CURRIED constructor encoding (Apply.. (Con CID) ..) — no gamma-side
# feature was needed; Con/Apply thread through pnorm/nateq/subt/freet already, and ffire leaves a
# non-constructor scrutinee STUCK, which is exactly what a refl certificate needs. The (fun FID ..) rule
# declarations are INLINED at each (f FID arg) site as (Fapp arg (Frule cidA bodyA) (Frule cidB bodyB)) —
# the same inlining precedent as prover.py --gamma's lemma handling. Reads the cert on stdin, prints the
# gamma expression on stdout; exits 1 on anything outside the refl-cert shape.
import sys


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
    h = t[0]
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
    if h == 'f':                                   # (f FID arg) -> Fapp with the FID's rules inlined
        (ca, ba), (cb, bb) = funs[t[1]]
        return '(Fapp %s (Frule %d %s) (Frule %d %s))' % (
            term(t[2], funs), ca, term(ba, funs), cb, term(bb, funs))
    raise SystemExit('untranslatable term head: %s' % h)


def prop(g, funs):
    if g[0] == 'All':
        return '(All %s)' % prop(g[1], funs)
    if g[0] == '=':
        return '(Eq %s %s)' % (term(g[1], funs), term(g[2], funs))
    raise SystemExit('untranslatable goal head: %s' % g[0])


def proof(p, funs):
    if p[0] == 'gen':
        return '(Gen %s)' % proof(p[1], funs)
    if p[0] == 'refl':
        return '(Refl %s)' % term(p[1], funs)
    raise SystemExit('untranslatable proof head: %s' % p[0])


def main():
    forms = parse(sys.stdin.read())
    funs = {}                                      # fid -> [(cidA, bodyA), (cidB, bodyB)]
    body = []
    for f in forms:
        if isinstance(f, list) and f and f[0] == 'data':
            continue                               # constructor ids need no declaration in the Con encoding
        if isinstance(f, list) and f and f[0] == 'fun':
            funs.setdefault(f[1], []).append((f[2], f[3]))
            continue
        body.append(f)
    if len(body) != 2:
        raise SystemExit('expected exactly <goal> <proof> after declarations')
    print('(check %s %s)' % (proof(body[1], funs), prop(body[0], funs)))


if __name__ == '__main__':
    main()
