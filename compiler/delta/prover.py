#!/usr/bin/env python3
# A PROOF-SEARCH FRONT LINE -- the Omega pattern in miniature (rungs/omega.md): untrusted automation
# discharges a goal and EMITS A CERTIFICATE the tiny trusted kernel (check.beta) validates. Given an
# intuitionistic propositional goal over the FULL connective set (`->`, `&`, `+`, `(bot)`), this searches
# a sound natural-deduction calculus (intro + elimination for each connective: lam/app, pair/fst/snd,
# inl/inr/case, absurd) and prints the check.beta certificate `<goal> <proof>`; the kernel checks it. The
# prover is UNTRUSTED: it is SOUND by construction (every rule it applies is a valid kernel typing rule,
# so check.beta accepts every proof it emits) -- exactly the "cleverness on the untrusted side, authority
# in the kernel" split. The search is memoised on (context proposition-set, goal), so it terminates
# without a depth bound and is polynomial in the subformula state space. Prints "unprovable" otherwise.
#
# Usage: prover.py "(-> (& P Q) P)"   ->   (-> (& P Q) P) (lam (& P Q) (fst (hyp 0)))
import sys

# ---- parse a goal into a tuple tree. Props: uppercase atoms, `->`/`&`/`+`/`(bot)`, and the first-order
# forms `(All P)` `(Exists P)` `(Pred n term)` `(Rel n term term)`. Terms: `z`, `(s term)`, `(v i)`. ----
def tokenize(s):
    return s.replace("(", " ( ").replace(")", " ) ").split()


def parse_term(tk):
    t = tk.pop(0)
    if t == "(":
        h = tk.pop(0)
        if h == "s":
            x = parse_term(tk)
            assert tk.pop(0) == ")"
            return ("s", x)
        if h == "v":
            i = int(tk.pop(0))
            assert tk.pop(0) == ")"
            return ("v", i)
        raise ValueError("bad term head: %s" % h)
    if t == "z":
        return ("z",)
    raise ValueError("bad term: %s" % t)


def parse(tokens):
    t = tokens.pop(0)
    if t == "(":
        head = tokens[0]
        if head in ("->", "&", "+"):
            tokens.pop(0)
            a = parse(tokens)
            b = parse(tokens)
            assert tokens.pop(0) == ")"
            return (head, a, b)
        if head == "bot":
            tokens.pop(0)
            assert tokens.pop(0) == ")"
            return ("bot",)
        if head in ("All", "Exists"):
            tokens.pop(0)
            body = parse(tokens)
            assert tokens.pop(0) == ")"
            return ("all" if head == "All" else "ex", body)
        if head == "Pred":
            tokens.pop(0)
            n = int(tokens.pop(0))
            term = parse_term(tokens)
            assert tokens.pop(0) == ")"
            return ("pred", n, term)
        if head == "Rel":
            tokens.pop(0)
            n = int(tokens.pop(0))
            a = parse_term(tokens)
            b = parse_term(tokens)
            assert tokens.pop(0) == ")"
            return ("rel", n, a, b)
        raise ValueError("bad prop head: %s" % head)
    return ("at", t)  # an atom name (a bare uppercase ident)


# Emission maps the search's NAMED individuals (eigenvariables, ("eig", k)) to check.beta's de Bruijn
# `(v i)`. `ib` lists the active eigenvar ids innermost-last (one per enclosing gen / unpack-handler binder);
# `depth` counts the prop's OWN internal quantifier binders we've descended under. An eigenvar that sits
# under `depth` internal binders renders at index depth + (its distance from the innermost outer binder),
# so an outer individual and an inner bound var never collide. A literal `(v i)` is always an internal
# binder reference (every free individual is an eigenvar in this representation), emitted as-is.
def beta_term(t, ib=(), depth=0):
    if t[0] == "z":
        return "z"
    if t[0] == "s":
        return "(s %s)" % beta_term(t[1], ib, depth)
    if t[0] == "eig":
        return "(v %d)" % (depth + len(ib) - 1 - list(ib).index(t[1]))
    return "(v %d)" % t[1]


def beta_prop(p, ib=(), depth=0):
    h = p[0]
    if h == "at":
        return p[1]
    if h == "bot":
        return "(bot)"
    if h == "pred":
        return "(Pred %d %s)" % (p[1], beta_term(p[2], ib, depth))
    if h == "rel":
        return "(Rel %d %s %s)" % (p[1], beta_term(p[2], ib, depth), beta_term(p[3], ib, depth))
    if h in ("all", "ex"):
        return "(%s %s)" % ("All" if h == "all" else "Exists", beta_prop(p[1], ib, depth + 1))
    return "(%s %s %s)" % (h, beta_prop(p[1], ib, depth), beta_prop(p[2], ib, depth))


def _subt(term, t, d):  # substitute the de Bruijn term-var `d` with `t` (shifted into d binders)
    if term[0] == "v":
        if term[1] == d:
            return _shift(t, d)
        return ("v", term[1] - 1) if term[1] > d else term
    if term[0] == "s":
        return ("s", _subt(term[1], t, d))
    return term  # z


def _shift(t, d):
    if t[0] == "v":
        return ("v", t[1] + d)
    if t[0] == "s":
        return ("s", _shift(t[1], d))
    return t


def subst0(p, t, d=0):  # substitute the outermost bound var (v0) of a body with term t
    h = p[0]
    if h == "pred":
        return ("pred", p[1], _subt(p[2], t, d))
    if h == "rel":
        return ("rel", p[1], _subt(p[2], t, d), _subt(p[3], t, d))
    if h in ("all", "ex"):
        return (h, subst0(p[1], t, d + 1))
    if h in ("->", "&", "+"):
        return (h, subst0(p[1], t, d), subst0(p[2], t, d))
    return p  # at, bot


def ground_terms(p, out):  # collect ground (var-free) candidate witness terms in a prop
    h = p[0]
    if h == "pred":
        _gt(p[2], out)
    elif h == "rel":
        _gt(p[2], out)
        _gt(p[3], out)
    elif h in ("all", "ex"):
        ground_terms(p[1], out)
    elif h in ("->", "&", "+"):
        ground_terms(p[1], out)
        ground_terms(p[2], out)


def _gt(t, out):
    if t[0] == "v":
        return  # not ground
    out.add(t)
    if t[0] == "s":
        _gt(t[1], out)


# ---- proof search over {-> , &}. Context entries are (prop, term) where `term` is a NAMED proof of
# `prop`; lam binders carry unique names, converted to de Bruijn indices at emit time. ----
_fresh = [0]


def fresh():
    _fresh[0] += 1
    return "h%d" % _fresh[0]


def saturate(ctx):
    # decompose every conjunction A&B in the context into A (fst) and B (snd), to a fixpoint
    out = list(ctx)
    changed = True
    while changed:
        changed = False
        for prop, term in list(out):
            if prop[0] == "&":
                fa = (prop[1], ("fst", term))
                sb = (prop[2], ("snd", term))
                for e in (fa, sb):
                    if e not in out:
                        out.append(e)
                        changed = True
    return out


def has(ctx, goal):
    for prop, term in ctx:
        if prop == goal:
            return term
    return None


# The search is MEMOISED on (context proposition-set, goal): the rules only ever ADD subformulas of the
# original goal to the context, so the reachable state space is finite and the search terminates WITHOUT a
# depth fuel. The memo caches FAILURES (provability depends only on which propositions are available, not on
# their proof-term names, so a failed (props, goal) stays failed) and doubles as a loop-check (an in-progress
# key re-entered = a cycle = no new proof). This turns an exponential re-exploration into a polynomial one.
# A node budget remains as a hard backstop so a pathological goal can never wedge the whole lattice run.
_budget = [0]
_depth = [0]      # current recursion depth; a cap keeps a pathological search from overrunning the C stack
_memo = {}
_DEPTH_CAP = 250  # each logical level is ~3 Python frames, so this stays well under the default ~1000-frame
# limit; the prover is sound-but-incomplete, so a
# search that would run deeper just yields "unprovable" (never a crash, never a false proof)


_candidates = []  # ground witness/instantiation terms for the quantifier rules, gathered per solve
_eigctr = [0]     # fresh-eigenvariable counter (reset per solve, so certs are deterministic)
_eigs = []        # active eigenvar ids (one per enclosing gen / unpack), innermost last
_opened = []      # existential PROPOSITIONS already opened on this branch (never re-open one -- a parent
# conjunction would otherwise regenerate it through saturation and unpack would loop with fresh eigenvars)


def fresh_eig():
    _eigctr[0] += 1
    return _eigctr[0]


def cand_terms():  # witness/instantiation candidates: in-scope eigenvariables FIRST, then goal ground terms.
    # Eigenvars-first matters: once an existential is opened to P(e), the witness/instance we want is almost
    # always that very e, so trying it first finds the proof shallow instead of exploring doomed ground-term
    # chains to (near) the depth cap.
    return [("eig", k) for k in reversed(_eigs)] + _candidates


def solve(goal, _fuel=None):
    _budget[0] = 200000
    _depth[0] = 0
    _memo.clear()
    _eigctr[0] = 0
    _eigs[:] = []
    _opened[:] = []
    cands = {("z",)}  # z is always available as a default witness
    ground_terms(goal, cands)
    _candidates[:] = list(cands)
    return prove([], goal)


def prove(ctx, goal):
    if _budget[0] <= 0 or _depth[0] >= _DEPTH_CAP:
        return None
    _budget[0] -= 1
    sat = saturate(ctx)
    key = (frozenset(p for p, _ in sat), goal, tuple(_eigs), frozenset(_opened))
    if key in _memo:  # already failed, or in progress (a cycle): no proof to be found this way
        return None
    _memo[key] = None  # tentatively mark unprovable (loop-break); cleared on success below
    _depth[0] += 1
    proof = _rules(sat, goal)
    _depth[0] -= 1
    if proof is not None:
        del _memo[key]
    return proof


def _rules(sat, goal):
    # axiom: the goal is already in (the saturation of) the context
    direct = has(sat, goal)
    if direct is not None:
        return direct
    # R&: prove each conjunct
    if goal[0] == "&":
        la = prove(sat, goal[1])
        lb = prove(sat, goal[2])
        if la is not None and lb is not None:
            return ("pair", la, lb)
    # R->: assume the antecedent, prove the consequent
    if goal[0] == "->":
        nm = fresh()
        body = prove(sat + [(goal[1], ("hyp", nm))], goal[2])
        if body is not None:
            return ("lam", nm, goal[1], body)
    # R+: prove one disjunct (inl carries the OTHER side's prop, per check.beta)
    if goal[0] == "+":
        la = prove(sat, goal[1])
        if la is not None:
            return ("inl", goal[2], la)
        rb = prove(sat, goal[2])
        if rb is not None:
            return ("inr", goal[1], rb)
    # R-forall (gen): introduce a FRESH eigenvariable for the bound var and prove the instantiated body.
    # The eigenvar is opaque (no rule can inspect its structure), so a proof of body[e] is parametric in
    # e -- exactly the universal. de Bruijn for nested binders is recovered at emit time from the eig stack.
    if goal[0] == "all":
        e = fresh_eig()
        _eigs.append(e)
        body = prove(sat, subst0(goal[1], ("eig", e)))
        _eigs.pop()
        if body is not None:
            return ("gen", e, body)
    # L-exists (unpack): OPEN an existential hypothesis with a fresh eigenvariable e, add body[e], and
    # continue. This is an INVERTIBLE (always-safe) left rule -- opening loses nothing, since body[e] is
    # strictly stronger than the existential -- so it runs BEFORE the non-invertible witness rule, which
    # keeps the productive proof shallow (otherwise the search explores doomed witness chains first and
    # recurses too deep). The eigenvariable condition (the conclusion must not mention e) holds
    # automatically: the goal predates e. We DROP the opened existential (never re-open it).
    for prop, term in sat:
        if prop[0] == "ex" and prop not in _opened:
            e = fresh_eig()
            nm = fresh()
            body_e = subst0(prop[1], ("eig", e))
            if has(sat, body_e) is not None:
                continue
            _eigs.append(e)
            _opened.append(prop)
            pf = prove(sat + [(body_e, ("hyp", nm))], goal)
            _opened.pop()
            _eigs.pop()
            if pf is not None:
                return ("unpack", term, prop[1], e, nm, pf)
    # R-exists (wit): supply a witness term (a ground term or an in-scope eigenvar) and prove the instance
    if goal[0] == "ex":
        for t in cand_terms():
            pf = prove(sat, subst0(goal[1], t))
            if pf is not None:
                return ("wit", goal[1], t, pf)
    # absurd: a falsity in context proves anything (ex falso quodlibet)
    bot = has(sat, ("bot",))
    if bot is not None and goal != ("bot",):
        return ("absurd", goal, bot)
    # L-forall (inst): instantiate a universal hypothesis with a candidate term (a NEW fact)
    for prop, term in sat:
        if prop[0] == "all":
            for t in cand_terms():
                sub = subst0(prop[1], t)
                if has(sat, sub) is None:
                    body = prove(sat + [(sub, ("inst", term, t))], goal)
                    if body is not None:
                        return body
    # L+: case-split on a disjunction hypothesis -- prove the goal under each disjunct
    for prop, term in sat:
        if prop[0] == "+":
            nl = fresh()
            la = prove(sat + [(prop[1], ("hyp", nl))], goal)
            if la is None:
                continue
            nr = fresh()
            rb = prove(sat + [(prop[2], ("hyp", nr))], goal)
            if rb is None:
                continue
            return ("case", term, ("lam", nl, prop[1], la), ("lam", nr, prop[2], rb))
    # L->: forward chaining (modus ponens). Only chain when the consequent is a NEW fact -- re-adding one
    # already in context loops; the memo also catches the rest.
    for prop, term in sat:
        if prop[0] == "->" and has(sat, prop[2]) is None:
            arg = prove(sat, prop[1])
            if arg is not None:
                body = prove(sat + [(prop[2], ("app", term, arg))], goal)
                if body is not None:
                    return body
    return None


def to_db(term, binders, ib=()):  # convert the named proof term to check.beta's de Bruijn syntax
    h = term[0]                    # `binders` = hypothesis names; `ib` = eigenvar ids (individual binders)
    if h == "hyp":
        return "(hyp %d)" % (len(binders) - 1 - binders.index(term[1]))
    if h == "lam":
        _, nm, prop, body = term
        return "(lam %s %s)" % (beta_prop(prop, ib), to_db(body, binders + [nm], ib))
    if h in ("fst", "snd"):
        return "(%s %s)" % (h, to_db(term[1], binders, ib))
    if h in ("inl", "inr", "absurd"):  # carry a PROP annotation, then a proof
        return "(%s %s %s)" % (h, beta_prop(term[1], ib), to_db(term[2], binders, ib))
    if h == "case":  # scrutinee + two lam branches
        return "(case %s %s %s)" % (to_db(term[1], binders, ib), to_db(term[2], binders, ib), to_db(term[3], binders, ib))
    if h == "gen":  # universal introduction: push the eigenvar -> a new innermost individual binder
        _, e, body = term
        return "(gen %s)" % to_db(body, binders, tuple(ib) + (e,))
    if h == "inst":  # universal elimination: a proof applied to a TERM
        return "(inst %s %s)" % (to_db(term[1], binders, ib), beta_term(term[2], ib))
    if h == "wit":  # existential introduction: body-prop, witness TERM, proof of the instance
        return "(wit %s %s %s)" % (beta_prop(term[1], ib), beta_term(term[2], ib), to_db(term[3], binders, ib))
    if h == "unpack":  # existential elimination: the handler is `(gen (lam body C))` -- one individual
        _, exterm, body, e, nm, pf = term   # binder (the witness) + one hypothesis binder (the body)
        ib2 = tuple(ib) + (e,)
        handler = "(gen (lam %s %s))" % (beta_prop(body, ib2), to_db(pf, binders + [nm], ib2))
        return "(unpack %s %s)" % (to_db(exterm, binders, ib), handler)
    return "(%s %s %s)" % (h, to_db(term[1], binders, ib), to_db(term[2], binders, ib))  # pair / app


def gen(n, seed):  # print n random {->,&} propositions over P..U -- the prover's fuzz feed
    import random
    random.seed(seed)
    atoms = ["P", "Q", "R", "S", "T", "U"]

    def rp(d):
        if d <= 0 or random.random() < 0.34:
            return ("at", random.choice(atoms))
        return (random.choice(("->", "&")), rp(d - 1), rp(d - 1))

    for _ in range(n):
        print(beta_prop(rp(random.randint(1, 4))))


def random_prop(d, rng):
    atoms = ["P", "Q", "R", "S", "T", "U"]
    r = rng.random()
    if d <= 0 or r < 0.30:
        return ("at", rng.choice(atoms))
    if r < 0.35:
        return ("bot",)
    return (rng.choice(("->", "&", "+")), random_prop(d - 1, rng), random_prop(d - 1, rng))


def batch(n, seed, fuel):
    # generate n random {->,&} goals; for every one the prover discharges, print "<goal>\t<cert>" on one
    # line (a single process -> the test pipes each cert to check.beta). The rest are silently dropped.
    import random
    rng = random.Random(seed)
    for _ in range(n):
        goal = random_prop(rng.randint(1, 4), rng)
        proof = solve(goal, fuel)
        if proof is not None:
            print("%s\t%s %s" % (beta_prop(goal), beta_prop(goal), to_db(proof, [])))


def main():
    if sys.argv[1] == "--gen":
        gen(int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 1)
        return
    if sys.argv[1] == "--batch":
        batch(int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 1, 16)
        return
    goal = parse(tokenize(sys.argv[1]))
    proof = solve(goal, int(sys.argv[2]) if len(sys.argv) > 2 else 16)
    if proof is None:
        print("unprovable")
    else:
        print("%s %s" % (beta_prop(goal), to_db(proof, [])))


if __name__ == "__main__":
    main()
