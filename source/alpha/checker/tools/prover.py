#!/usr/bin/env python3
# A PROOF-SEARCH FRONT LINE -- the Omega pattern in miniature (omega_toolchain.md): untrusted automation
# discharges a goal and EMITS A CERTIFICATE the tiny trusted kernel (check.beta) validates. It covers the
# FULL intuitionistic propositional fragment (`->`, `&`, `+`, `(bot)`: lam/app, pair/fst/snd, inl/inr/case,
# absurd), the FIRST-ORDER fragment (predicates/relations over terms, ∀/∃ with intro AND elim --
# gen/inst/wit/unpack over a uniform eigenvariable scheme), and EQUALITY (`(= a b)` over Peano terms
# z/s/p/m: reflexivity up to the kernel's own term conversion, plus a conversion-aware axiom). It searches a
# sound natural-deduction calculus and prints the check.beta certificate `<goal> <proof>`; the kernel checks
# it. The prover is UNTRUSTED: SOUND by construction (every rule it applies is a valid kernel typing rule, so
# check.beta accepts every proof it emits) -- the "cleverness on the untrusted side, authority in the kernel"
# split. The propositional search is memoised on (context proposition-set, goal) and polynomial; a depth cap
# + node budget backstop the (eigenvar-rich) first-order space. Prints "unprovable" otherwise.
#
# Usage: prover.py "(-> (& P Q) P)"   ->   (-> (& P Q) P) (lam (& P Q) (fst (hyp 0)))
import sys

sys.setrecursionlimit(8000)  # headroom; the depth cap + rewrite size guard keep us far below this in practice

# ---- parse a goal into a tuple tree. Props: uppercase atoms, `->`/`&`/`+`/`(bot)`, the first-order forms
# `(All P)` `(Exists P)` `(Pred n term)` `(Rel n term term)`, and equality `(= term term)`. Terms: `z`,
# `(s term)`, `(p term term)` (plus), `(m term term)` (mult), `(v i)` (de Bruijn individual var). ----
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
        if h in ("p", "m"):  # p = Peano plus, m = Peano mult (binary)
            a = parse_term(tk)
            b = parse_term(tk)
            assert tk.pop(0) == ")"
            return (h, a, b)
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
        if head == "=":
            tokens.pop(0)
            a = parse_term(tokens)
            b = parse_term(tokens)
            assert tokens.pop(0) == ")"
            return ("=", a, b)
        if head in ("Le", "Lt"):  # inequality SUGAR, desugared to a Peano existential (no kernel `<` needed):
            tokens.pop(0)         #   a <= b  :=  exists k. a + k     = b      (k >= 0)
            a = parse_term(tokens)  # a <  b  :=  exists k. a + (s k) = b      (b is at least a+1)
            b = parse_term(tokens)  # a,b move under the new binder, so shift their free vars up by one.
            assert tokens.pop(0) == ")"
            rhs = ("s", ("v", 0)) if head == "Lt" else ("v", 0)
            return ("ex", ("=", ("p", _shift(a, 1), rhs), _shift(b, 1)))
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
    if t[0] in ("p", "m"):  # Peano plus / mult
        return "(%s %s %s)" % (t[0], beta_term(t[1], ib, depth), beta_term(t[2], ib, depth))
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
    if h == "=":
        return "(= %s %s)" % (beta_term(p[1], ib, depth), beta_term(p[2], ib, depth))
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
    if term[0] in ("p", "m"):
        return (term[0], _subt(term[1], t, d), _subt(term[2], t, d))
    return term  # z, eig (an eigenvariable is opaque -- no de Bruijn var inside it)


def _shift(t, d):
    if t[0] == "v":
        return ("v", t[1] + d)
    if t[0] == "s":
        return ("s", _shift(t[1], d))
    if t[0] in ("p", "m"):
        return (t[0], _shift(t[1], d), _shift(t[2], d))
    return t


def subst0(p, t, d=0):  # substitute the outermost bound var (v0) of a body with term t
    h = p[0]
    if h == "pred":
        return ("pred", p[1], _subt(p[2], t, d))
    if h == "rel":
        return ("rel", p[1], _subt(p[2], t, d), _subt(p[3], t, d))
    if h == "=":
        return ("=", _subt(p[1], t, d), _subt(p[2], t, d))
    if h in ("all", "ex"):
        return (h, subst0(p[1], t, d + 1))
    if h in ("->", "&", "+"):
        return (h, subst0(p[1], t, d), subst0(p[2], t, d))
    return p  # at, bot


def _subt_keep(term, t, d):  # substitute de Bruijn var d with t, but KEEP the binder (no decrement)
    if term[0] == "v":
        return _shift(t, d) if term[1] == d else term
    if term[0] == "s":
        return ("s", _subt_keep(term[1], t, d))
    if term[0] in ("p", "m"):
        return (term[0], _subt_keep(term[1], t, d), _subt_keep(term[2], t, d))
    return term


def subst0_keep(p, t, d=0):  # P[v0 := t] keeping the binder -- builds P(s n) for natind's step from motive P
    h = p[0]
    if h == "pred":
        return ("pred", p[1], _subt_keep(p[2], t, d))
    if h == "rel":
        return ("rel", p[1], _subt_keep(p[2], t, d), _subt_keep(p[3], t, d))
    if h == "=":
        return ("=", _subt_keep(p[1], t, d), _subt_keep(p[2], t, d))
    if h in ("all", "ex"):
        return (h, subst0_keep(p[1], t, d + 1))
    if h in ("->", "&", "+"):
        return (h, subst0_keep(p[1], t, d), subst0_keep(p[2], t, d))
    return p


def ground_terms(p, out):  # collect ground (var-free) candidate witness terms in a prop
    h = p[0]
    if h == "pred":
        _gt(p[2], out)
    elif h in ("rel", "="):
        _gt(p[1 if h == "=" else 2], out)
        _gt(p[2 if h == "=" else 3], out)
    elif h in ("all", "ex"):
        ground_terms(p[1], out)
    elif h in ("->", "&", "+"):
        ground_terms(p[1], out)
        ground_terms(p[2], out)


def _gt(t, out):  # a term is a candidate only if it is ground (contains no free de Bruijn var)
    if t[0] == "v":
        return
    if t[0] in ("p", "m"):
        _gt(t[1], out)
        _gt(t[2], out)
        if _ground(t):
            out.add(t)
        return
    out.add(t)
    if t[0] == "s":
        _gt(t[1], out)


# ---- A goal may carry FREE individual vars (e.g. x,y in x<y), implicitly universally quantified. We close
# them into fresh eigenvariables before search: every individual then becomes an eigenvar (opaque parameter)
# or a prop-internal bound var, so the eigenvar/de-Bruijn emission machinery (which shifts correctly under
# gen/unpack binders) handles them uniformly. Without this a free var emitted under an eigenvar binder is not
# shifted -> a malformed (kernel-rejected) certificate. ----
def _max_free_term(t, d):  # 1 + highest free-var LEVEL (index - binder depth) in a term; 0 if none free
    if t[0] == "v":
        return t[1] - d + 1 if t[1] >= d else 0
    if t[0] == "s":
        return _max_free_term(t[1], d)
    if t[0] in ("p", "m"):
        return max(_max_free_term(t[1], d), _max_free_term(t[2], d))
    return 0


def _max_free(p, d=0):
    h = p[0]
    if h == "pred":
        return _max_free_term(p[2], d)
    if h == "rel":
        return max(_max_free_term(p[2], d), _max_free_term(p[3], d))
    if h == "=":
        return max(_max_free_term(p[1], d), _max_free_term(p[2], d))
    if h in ("all", "ex"):
        return _max_free(p[1], d + 1)
    if h in ("->", "&", "+"):
        return max(_max_free(p[1], d), _max_free(p[2], d))
    return 0


def _close_term(t, eigs, d):  # replace each free var (level i-d) with eigs[i-d]
    if t[0] == "v":
        return ("eig", eigs[t[1] - d]) if t[1] >= d else t
    if t[0] == "s":
        return ("s", _close_term(t[1], eigs, d))
    if t[0] in ("p", "m"):
        return (t[0], _close_term(t[1], eigs, d), _close_term(t[2], eigs, d))
    return t


def _close_free(p, eigs, d=0):
    h = p[0]
    if h == "pred":
        return ("pred", p[1], _close_term(p[2], eigs, d))
    if h == "rel":
        return ("rel", p[1], _close_term(p[2], eigs, d), _close_term(p[3], eigs, d))
    if h == "=":
        return ("=", _close_term(p[1], eigs, d), _close_term(p[2], eigs, d))
    if h in ("all", "ex"):
        return (h, _close_free(p[1], eigs, d + 1))
    if h in ("->", "&", "+"):
        return (h, _close_free(p[1], eigs, d), _close_free(p[2], eigs, d))
    return p


def _ground(t):  # True if the term has no free de Bruijn var (eigenvariables count as ground atoms)
    if t[0] == "v":
        return False
    if t[0] == "s":
        return _ground(t[1])
    if t[0] in ("p", "m"):
        return _ground(t[1]) and _ground(t[2])
    return True  # z, eig


def nf(t):  # normal form, mirroring check.beta's `normalize` for the z/s/p/m term fragment EXACTLY. Every
    # rewrite used is a kernel rule, so nf(a)==nf(b) ==> the kernel's conversion equates a and b -> (refl a)
    # is ACCEPTED for goal (= a b). Matching the kernel's weak-head stuck forms avoids spurious rejects.
    h = t[0]
    if h == "s":
        return ("s", nf(t[1]))
    if h == "p":                       # p z b => b ; p (s x) b => s (p x b) ; else stuck (both args normal)
        a = nf(t[1])
        b = nf(t[2])
        if a[0] == "z":
            return b
        if a[0] == "s":
            return ("s", nf(("p", a[1], b)))
        return ("p", a, b)
    if h == "m":                       # m z y => z ; m (s x) y => p y (m x y) ; else stuck (2nd arg RAW)
        a = nf(t[1])
        if a[0] == "z":
            return ("z",)
        if a[0] == "s":
            return nf(("p", t[2], ("m", a[1], t[2])))
        return ("m", a, t[2])
    return t                           # z, v, eig -- already normal


def nf_prop(p):  # normalize every term inside a proposition (the kernel's type_eq compares props up to this)
    h = p[0]
    if h == "pred":
        return ("pred", p[1], nf(p[2]))
    if h == "rel":
        return ("rel", p[1], nf(p[2]), nf(p[3]))
    if h == "=":
        return ("=", nf(p[1]), nf(p[2]))
    if h in ("all", "ex"):
        return (h, nf_prop(p[1]))
    if h in ("->", "&", "+"):
        return (h, nf_prop(p[1]), nf_prop(p[2]))
    return p  # at, bot


# ---- equality REWRITING via the kernel's eqelim (Leibniz transport). To prove goal G using e:(= X Y),
# ABSTRACT the occurrences of one side as a motive M (M has the rewrite-hole at de Bruijn 0), then prove the
# motive applied to the other side. eqelim takes pf_eq:(= X Y) and pf_pa:M[X] and yields M[Y]; so to land on
# G = M[Y] we discharge the subgoal M[X]. Both orientations (and so sym/trans/congruence/transport) fall out
# of this single rule. The rewritten term must be GROUND (no free de Bruijn var) so G's binders can't capture
# it. ----
def term_size(t):
    if t[0] in ("z", "v", "eig"):
        return 1
    if t[0] == "s":
        return 1 + term_size(t[1])
    return 1 + term_size(t[1]) + term_size(t[2])  # p, m


def prop_size(p):  # total term-node count in a proposition -- the rewrite size guard's metric
    h = p[0]
    if h == "pred":
        return term_size(p[2])
    if h == "rel":
        return term_size(p[2]) + term_size(p[3])
    if h == "=":
        return term_size(p[1]) + term_size(p[2])
    if h in ("all", "ex"):
        return prop_size(p[1])
    if h in ("->", "&", "+"):
        return prop_size(p[1]) + prop_size(p[2])
    return 0  # at, bot


def _match_term(pat, t, subst):  # first-order MATCH: bind pattern holes (v i) to ground subterms of t; the
    if pat[0] == "v":            # pattern's de Bruijn vars are the lemma's universally-bound variables
        i = pat[1]
        if i in subst:
            return subst if subst[i] == t else None
        s = dict(subst)
        s[i] = t
        return s
    if pat[0] != t[0]:
        return None
    if pat[0] in ("z", "eig"):
        return subst if pat == t else None
    if pat[0] == "s":
        return _match_term(pat[1], t[1], subst)
    if pat[0] in ("p", "m"):
        s = _match_term(pat[1], t[1], subst)
        return _match_term(pat[2], t[2], s) if s is not None else None
    return None


def _fill(pat, subst):  # apply a hole substitution to a pattern term
    if pat[0] == "v":
        return subst[pat[1]]
    if pat[0] == "s":
        return ("s", _fill(pat[1], subst))
    if pat[0] in ("p", "m"):
        return (pat[0], _fill(pat[1], subst), _fill(pat[2], subst))
    return pat


def _subterms(t, out):  # every compound subterm of a term (candidates to match a lemma's LHS against)
    out.append(t)
    if t[0] == "s":
        _subterms(t[1], out)
    elif t[0] in ("p", "m"):
        _subterms(t[1], out)
        _subterms(t[2], out)


def _prop_subterms(p):  # every term-subterm appearing in a proposition
    out = []
    h = p[0]
    if h == "pred":
        _subterms(p[2], out)
    elif h == "rel":
        _subterms(p[2], out)
        _subterms(p[3], out)
    elif h == "=":
        _subterms(p[1], out)
        _subterms(p[2], out)
    elif h in ("all", "ex"):
        out += _prop_subterms(p[1])
    elif h in ("->", "&", "+"):
        out += _prop_subterms(p[1]) + _prop_subterms(p[2])
    return out


def _slack_path(sat, A, C, maxlen=4):
    # The context's `+`-equality facts form a graph: a fact (= (p X D) Y) is an edge X --slack D--> Y.
    # BFS for a path A -> ... -> C and return its slacks in order (so A ≤ C with witness = their sum). This
    # generalises 2-step transitivity to arbitrary-length bound chains (a≤b≤c≤d ⊢ a≤d).
    edges = {}
    for p, _ in sat:
        if p[0] == "=" and p[1][0] == "p":
            edges.setdefault(p[1][1], []).append((p[1][2], p[2]))
    queue = [(A, [])]
    seen = {A}
    while queue:
        node, slacks = queue.pop(0)
        if len(slacks) >= maxlen:
            continue
        for d, y in edges.get(node, []):
            ns = slacks + [d]
            if y == C:
                return ns
            if y not in seen:
                seen.add(y)
                queue.append((y, ns))
    return None


def _sum_right(slacks):  # right-associated sum d1+(d2+(...+dn)) -- the witness for an ≤-chain's path
    w = slacks[-1]
    for d in reversed(slacks[:-1]):
        w = ("p", d, w)
    return w


def occurs_term(t, b):
    if t == b:
        return True
    if t[0] == "s":
        return occurs_term(t[1], b)
    if t[0] in ("p", "m"):
        return occurs_term(t[1], b) or occurs_term(t[2], b)
    return False


def occurs_prop(p, b):
    h = p[0]
    if h == "pred":
        return occurs_term(p[2], b)
    if h == "rel":
        return occurs_term(p[2], b) or occurs_term(p[3], b)
    if h == "=":
        return occurs_term(p[1], b) or occurs_term(p[2], b)
    if h in ("all", "ex"):
        return occurs_prop(p[1], b)
    if h in ("->", "&", "+"):
        return occurs_prop(p[1], b) or occurs_prop(p[2], b)
    return False


def abstract_term(t, b, d):  # replace ground term `b` with the hole (v d); shift free vars up past the hole
    if t == b:
        return ("v", d)
    if t[0] == "v":
        return ("v", t[1] + 1) if t[1] >= d else t
    if t[0] == "s":
        return ("s", abstract_term(t[1], b, d))
    if t[0] in ("p", "m"):
        return (t[0], abstract_term(t[1], b, d), abstract_term(t[2], b, d))
    return t  # z, eig


def abstract_prop(p, b, d=0):  # the motive: G with `b`-occurrences turned into the de Bruijn-0 hole
    h = p[0]
    if h == "pred":
        return ("pred", p[1], abstract_term(p[2], b, d))
    if h == "rel":
        return ("rel", p[1], abstract_term(p[2], b, d), abstract_term(p[3], b, d))
    if h == "=":
        return ("=", abstract_term(p[1], b, d), abstract_term(p[2], b, d))
    if h in ("all", "ex"):
        return (h, abstract_prop(p[1], b, d + 1))
    if h in ("->", "&", "+"):
        return (h, abstract_prop(p[1], b, d), abstract_prop(p[2], b, d))
    return p  # at, bot


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
_IND_CAP = 2          # max natind nesting depth (most arithmetic needs single induction; 2 covers a little more)
_DEPTH_CAP = 250  # each logical level is ~3 Python frames, so this stays well under the default ~1000-frame
# limit; the prover is sound-but-incomplete, so a
# search that would run deeper just yields "unprovable" (never a crash, never a false proof)


_candidates = []  # ground witness/instantiation terms for the quantifier rules, gathered per solve
_eigctr = [0]     # fresh-eigenvariable counter (reset per solve, so certs are deterministic)
_eigs = []        # active eigenvar ids (one per enclosing gen / unpack), innermost last
_base_ib = []     # eigenvars standing in for the goal's FREE individual vars (the implicit outermost binders)
_opened = []      # existential PROPOSITIONS already opened on this branch (never re-open one -- a parent
# conjunction would otherwise regenerate it through saturation and unpack would loop with fresh eigenvars)
_ind_depth = [0]  # current natind nesting; capped so induction can't recurse without bound
_rw_cap = [0]     # ABSOLUTE size ceiling for an equality-rewrite subgoal (set per solve from the goal size),
# so a growing-direction rewrite can't ratchet without bound (the relative guard alone lets it spiral)


def fresh_eig():
    _eigctr[0] += 1
    return _eigctr[0]


def cand_terms():  # witness/instantiation candidates: in-scope eigenvariables FIRST, then goal ground terms.
    # Eigenvars-first matters: once an existential is opened to P(e), the witness/instance we want is almost
    # always that very e, so trying it first finds the proof shallow instead of exploring doomed ground-term
    # chains to (near) the depth cap. We also offer the SUCCESSOR of each eigenvar: weakening a strict bound
    # (a<b -> a<=b) opens `a + s k = b` and needs witness `s k` for the `a + k' = b` goal -- a tiny, bounded
    # enrichment (one per in-scope eigenvar) that the bare-term candidate set can't synthesise.
    eigs = [("eig", k) for k in reversed(_eigs)]
    return eigs + [("s", e) for e in eigs] + _candidates


# ---- arithmetic LEMMA LIBRARY: a few universal facts (proved once by the prover itself, via natind) that a
# goal can REUSE instead of re-deriving inline. Emitted as a `(def N prop proof)` prelude and cited by
# `(use N)`. Two-phase solve: try WITHOUT lemmas first (so simple/closed goals stay lean), and only retry WITH
# the library seeded as hypotheses when an arithmetic goal fails -- the multi-lemma case (y<=x+y, etc.). ----
LEMMA_PROPS = [
    ("all", ("=", ("p", ("v", 0), ("z",)), ("v", 0))),                                    # 0: x + 0 = x
    ("all", ("all", ("=", ("p", ("v", 1), ("s", ("v", 0))), ("s", ("p", ("v", 1), ("v", 0)))))),  # 1: x+(s y)=s(x+y)
    ("all", ("all", ("=", ("p", ("v", 1), ("v", 0)), ("p", ("v", 0), ("v", 1))))),        # 2: x + y = y + x
    ("all", ("all", ("all", ("=", ("p", ("p", ("v", 2), ("v", 1)), ("v", 0)),
                                   ("p", ("v", 2), ("p", ("v", 1), ("v", 0))))))),         # 3: (x+y)+z = x+(y+z)
    ("all", ("all", ("=", ("p", ("s", ("v", 1)), ("v", 0)), ("s", ("p", ("v", 1), ("v", 0)))))),  # 4: (s x)+y = s(x+y)
    ("all", ("all", ("all", ("=", ("m", ("p", ("v", 2), ("v", 1)), ("v", 0)),
                                   ("p", ("m", ("v", 2), ("v", 0)), ("m", ("v", 1), ("v", 0))))))),  # 5: (x+y)*a = x*a+y*a
]
_COMM = 2   # add-commutes' index in LEMMA_PROPS -- used by the additive two-bound (interval) composition
_ASSOC = 3  # add-assoc's index in LEMMA_PROPS -- used by the directed sum-chain (transitivity) rule
_SUCCL = 4  # add-succ-left ((s x)+y = s(x+y), refl-provable) -- lets the STRICT (<) chain peel the `s` slot
_RDIST = 5  # right-distributivity ((x+y)*a = x*a+y*a) -- the mult lemma that closes the mult-assoc natind step
_both_orient = [False]  # when set, the lemma-rewrite matches a lemma's RHS too (needed to use add-assoc as
# a+(i+j) -> (a+i)+j). Kept OFF in the general search (it bloats it) and turned ON only for the directed rule's
# small focused sub-proof, so the general phase-2 search is unchanged.
_lemma_cache = [None]   # built once: [(prop, proof_term), ...]
_used_lemmas = [[]]     # the lemmas seeded into the LAST solve's certificate (emitted as the def-prelude)
_active_lemmas = []     # peeled equality lemmas in scope: (arity, lhs, rhs, use_index) -- used by DIRECTED
# matching (instantiate a lemma only where its LHS matches a goal subterm), never blind inst (which explodes)
_induction_on = [False]  # natind only runs in phase 2 -- a DOOMED induction is expensive, and phase 1 should
# fail fast so the (arithmetic) goal reaches phase 2; every induction-needing goal is arithmetic, so none is lost


def _term_has_pm(t):
    if t[0] in ("p", "m"):
        return True
    if t[0] == "s":
        return _term_has_pm(t[1])
    return False


def _has_arith(p):  # does the proposition mention a plus/mult term? (then the lemma library may help)
    h = p[0]
    if h == "pred":
        return _term_has_pm(p[2])
    if h == "rel":
        return _term_has_pm(p[2]) or _term_has_pm(p[3])
    if h == "=":
        return _term_has_pm(p[1]) or _term_has_pm(p[2])
    if h in ("all", "ex"):
        return _has_arith(p[1])
    if h in ("->", "&", "+"):
        return _has_arith(p[1]) or _has_arith(p[2])
    return False


def _term_drives(t, d):  # does (v d) sit in a RECURSION-DRIVING position of term t? Peano `p`/`m` reduce by
    if t[0] == "s":       # destructing ONLY their FIRST argument (p (s x) b => s(p x b), m (s x) y => p y (m x y)),
        return _term_drives(t[1], d)   # so induction on a var makes progress only if that var lies on the first-argument
    if t[0] in ("p", "m"):             # SPINE -- never inside a second argument (which a stuck outer redex won't reduce).
        return t[1] == ("v", d) or _term_drives(t[1], d)   # Descending only into arg 1 is what prunes the doomed search:
    return False                       # inducting on `y`/`x` in a*(x+y) is futile while `m` is stuck on the opaque `a`.


def _drives(p, d=0):  # is the motive's induction variable (v d, shifting under inner binders) in a driving
    h = p[0]           # position anywhere in p? If NOT, natind on it cannot reduce the goal -- so skip it (the
    if h == "pred":    # explosive doomed case: e.g. inducting on `y` in a*(x+y) where `m` recurses on `a`, not y).
        return _term_drives(p[2], d)
    if h == "rel":
        return _term_drives(p[2], d) or _term_drives(p[3], d)
    if h == "=":
        return _term_drives(p[1], d) or _term_drives(p[2], d)
    if h in ("all", "ex"):
        return _drives(p[1], d + 1)
    if h in ("->", "&", "+"):
        return _drives(p[1], d) or _drives(p[2], d)
    return False


def _term_has_m(t):  # does term t mention a mult?
    if t[0] == "m":
        return True
    if t[0] == "s":
        return _term_has_m(t[1])
    if t[0] == "p":
        return _term_has_m(t[1]) or _term_has_m(t[2])
    return False


def _prop_has_m(p):  # does proposition p mention a mult term anywhere?
    h = p[0]
    if h == "pred":
        return _term_has_m(p[2])
    if h == "rel":
        return _term_has_m(p[2]) or _term_has_m(p[3])
    if h == "=":
        return _term_has_m(p[1]) or _term_has_m(p[2])
    if h in ("all", "ex"):
        return _prop_has_m(p[1])
    if h in ("->", "&", "+"):
        return _prop_has_m(p[1]) or _prop_has_m(p[2])
    return False


def _term_drives_mult(t, d, seen_m=False):  # is (v d) on a first-argument spine that passes through a MULT?
    if t == ("v", d):                        # `m` is the costly recursion: gen-freezing the var atop a stuck mult
        return seen_m                         # is what explodes the parametric search, so it's the natind-FIRST trigger.
    if t[0] == "s" or t[0] == "p":
        return _term_drives_mult(t[1], d, seen_m)
    if t[0] == "m":
        return _term_drives_mult(t[1], d, True)
    return False


def _is_ueq(p):  # is p a bare UNIVERSAL EQUATION (∀…∀. lhs = rhs)? -- the shape natind-first targets (an
    while p[0] == "all":  # implication/contract goal is proved by gen + hypothesis discharge, not induction).
        p = p[1]
    return p[0] == "="


def _drives_mult(p, d=0):  # like _drives but only counts a variable that drives a MULT recursion (the doomed-gen
    h = p[0]               # case worth inducting on FIRST). Pure-`+` goals (interchange, comm, assoc) stay gen-first.
    if h == "pred":
        return _term_drives_mult(p[2], d)
    if h == "rel":
        return _term_drives_mult(p[2], d) or _term_drives_mult(p[3], d)
    if h == "=":
        return _term_drives_mult(p[1], d) or _term_drives_mult(p[2], d)
    if h in ("all", "ex"):
        return _drives_mult(p[1], d + 1)
    if h in ("->", "&", "+"):
        return _drives_mult(p[1], d) or _drives_mult(p[2], d)
    return False


def _setup(goal):  # reset per-solve state, close the goal's free vars to eigenvars, seed candidates
    _budget[0] = 30000   # real proofs are shallow; a smaller budget makes doomed searches give up fast (the
    #                      equality rewrite + conversion axiom raised the per-node cost) -- sound, just less complete
    _depth[0] = 0
    _memo.clear()
    _eigctr[0] = 0
    _opened[:] = []
    _ind_depth[0] = 0
    nfree = _max_free(goal)
    free_eigs = [fresh_eig() for _ in range(nfree)]
    base = list(reversed(free_eigs))
    _eigs[:] = base
    _base_ib[:] = base
    cgoal = _close_free(goal, free_eigs)
    _rw_cap[0] = prop_size(cgoal) + 30  # rewrites may grow a bit (transitivity through a larger term) but not unboundedly
    cands = {("z",)}  # z is always available as a default witness
    ground_terms(cgoal, cands)
    _candidates[:] = list(cands)
    return cgoal


def _peel(prop):  # peel a universal-equality lemma to (arity, lhs, rhs) -- the bound vars become v0..v(arity-1)
    arity = 0
    while prop[0] == "all":
        arity += 1
        prop = prop[1]
    return (arity, prop[1], prop[2]) if prop[0] == "=" else None


def build_lemmas():  # prove each library lemma ONCE via natind, cache it. Built INCREMENTALLY: when proving
    if _lemma_cache[0] is not None:   # lemma i, the already-built lemmas 0..i-1 are available as DIRECTED rewrites
        return _lemma_cache[0]        # (not blind inst-hyps, which explode) -- so a later lemma may cite an earlier
    lemmas = []                       # one. right-distributivity (5), e.g., needs add-assoc (3) in its natind step.
    _induction_on[0] = True   # the lemmas themselves are proved by induction
    for prop in LEMMA_PROPS:
        cg = _setup(prop)
        _active_lemmas[:] = []          # FIRST try standalone -- the additive lemmas are self-contained, and
        pf = prove([], cg)              # seeding the library here would only DILUTE their directed-matching search.
        if pf is None:                  # only if standalone fails, RETRY with the already-built lemmas available
            cg = _setup(prop)           # as directed rewrites (right-distributivity needs add-assoc in its step).
            _active_lemmas[:] = [_peel(p) + (j,) for j, (p, _) in enumerate(lemmas) if _peel(p)]
            pf = prove([], cg)
        if pf is not None:  # only keep lemmas the prover (hence the kernel) actually proves -- stays sound
            lemmas.append((prop, pf))
    _active_lemmas[:] = []
    _induction_on[0] = False
    _lemma_cache[0] = lemmas
    return lemmas


def solve(goal, _fuel=None):
    _active_lemmas[:] = []
    _induction_on[0] = False         # phase 1: no induction (a doomed induction is slow) and no lemmas
    cgoal = _setup(goal)
    pf = prove([], cgoal)
    if pf is not None:
        _used_lemmas[0] = []
        return pf
    if _has_arith(cgoal):            # phase 2: retry with INDUCTION + the lemma library (directed matching)
        lemmas = build_lemmas()
        cgoal = _setup(goal)         # re-close (fresh eigenvars) now that the library is built
        _active_lemmas[:] = [_peel(p) + (i,) for i, (p, _) in enumerate(lemmas) if _peel(p)]
        _induction_on[0] = True
        pf = prove([], cgoal)
        _active_lemmas[:] = []
        _induction_on[0] = False
        if pf is not None:
            _used_lemmas[0] = lemmas
            return pf
    _used_lemmas[0] = []
    return None


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


def _try_natind(sat, goal):  # prove (All P) by Peano INDUCTION: base P(0) + step (All P(n)->P(s n)). The step
    motive = goal[1]          # is closed by gen + the induction hypothesis (a local universal-equation hyp now
    base_goal = subst0(motive, ("z",))                          # usable as a directed rewrite -- see L-eqrewrite).
    step_goal = ("all", ("->", motive, subst0_keep(motive, ("s", ("v", 0)))))
    # base/step introduce NEW ground terms (e.g. the base's (s z) witness) absent from the original goal's
    # candidate set -- gather them so the existential-witness search can find them.
    saved = list(_candidates)
    extra = set(_candidates)
    ground_terms(base_goal, extra)
    ground_terms(step_goal, extra)
    _candidates[:] = list(extra)
    _ind_depth[0] += 1
    base = prove(sat, base_goal)
    step = prove(sat, step_goal) if base is not None else None
    _ind_depth[0] -= 1
    _candidates[:] = saved
    if base is not None and step is not None:
        return ("natind", motive, base, step)
    return None


# universal strict-order irreflexivity, ∀a. a<a -> ⊥ (desugared), proven on demand and cited by the
# order-cycle rule below. Built via parse so its de Bruijn form matches exactly what the search produces.
_IRREFL = parse(tokenize("(All (-> (Lt (v 0) (v 0)) (bot)))"))
_CANCEL0 = parse(tokenize("(All (All (-> (= (p (v 1) (v 0)) (v 1)) (= (v 0) z))))"))   # a+m=a -> m=0
_POSIT  = parse(tokenize("(All (All (-> (= (p (v 1) (v 0)) z) (= (v 1) z))))"))          # a+b=0 -> a=0 (positivity)
_ACR = parse(tokenize("(All (All (All (-> (= (p (v 2) (v 0)) (p (v 1) (v 0))) (= (v 2) (v 1))))))"))  # a+c=b+c -> a=b


def _add_base(p):  # an additive-equality fact `(= (p A K) B)` -> (A, K, B); else None
    if p[0] == "=" and p[1][0] == "p":
        return (p[1][1], p[1][2], p[2])
    return None


def _strict_base(p):  # if p is a strict-Lt fact `(= (p A (s K)) B)` (what unpacking `A < B` yields), return
    if (p[0] == "=" and p[1][0] == "p" and p[1][2][0] == "s"):  # (A, B); else None. A<B means ∃k. A+(s k)=B.
        return (p[1][1], p[2])
    return None


def _cancel0_shape(goal):  # a CANCEL0-like goal: ∀…∀. (eq -> … -> eq) where the OUTERMOST bound var is a BARE
    n = 0; p = goal            # side of some ANTECEDENT equation (e.g. a+m=a, RHS bare `a`). Inducting on that var
    while p[0] == "all":       # UP FRONT (natind-first) avoids the doomed gen-first budget burn that otherwise
        n += 1; p = p[1]       # starves the real natind-on-that-var. Distinguishes CANCEL0 (outer var bare in an
    if n == 0 or p[0] != "->": # antecedent) from add-cancel-right (outer var only inside `p`, so its right
        return False           # induction var is the INNER common addend, correctly reached by gen+fallback).
    iv = ("v", n - 1)
    while p[0] == "->":
        if p[1][0] == "=" and (p[1][1] == iv or p[1][2] == iv):
            q = goal
            while q[0] == "all": q = q[1]
            while q[0] == "->":          # and every antecedent + the conclusion must be an equation (so Lt/Le=ex
                if q[1][0] != "=": return False   # contract goals, proved by gen+sum-witness, are excluded)
                q = q[2]
            return q[0] == "="
        p = p[2]
    return False


def _rules(sat, goal):
    # axiom: the goal is already in (the saturation of) the context
    direct = has(sat, goal)
    if direct is not None:
        return direct
    # refl: an equality goal whose two sides share a normal form (under the kernel's own term reduction).
    # (refl a) : (= a a), and the kernel's conversion equates a with b exactly when nf(a)==nf(b), so it
    # accepts (refl a) : (= a b). This discharges definitional arithmetic -- e.g. (= (p (s z) (s z)) (s (s z))).
    if goal[0] == "=" and nf(goal[1]) == nf(goal[2]):
        return ("refl", goal[1])
    # conversion axiom: a hypothesis EQUAL TO THE GOAL up to the kernel's term conversion. type_eq normalizes,
    # so (hyp i) : H is accepted for goal G when nf_prop(H)==nf_prop(G) -- e.g. a hyp P(1+1) discharges goal
    # P(2). Only fires when the structural axiom missed (an exact match already returned above) and the goal
    # actually carries terms (it differs from structural equality only for pred/rel/= -bearing props).
    if goal[0] in ("pred", "rel", "=", "->", "&", "+", "all", "ex"):
        ng = nf_prop(goal)
        for prop, term in sat:
            if prop != goal and nf_prop(prop) == ng:
                return term
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
    # natind FIRST on a TOP-LEVEL (depth 0) universal-EQUATION goal whose OUTER variable drives the recursion.
    # For such a goal `gen` is doomed -- it freezes the recursion variable into an opaque eigenvar, leaving a
    # stuck redex (m e …) that no INNER induction can unstick, so the parametric search explodes into unbounded
    # doomed nested induction before failing. Inducting on the driving variable up front avoids that whole
    # subtree (both distributivity directions, hence mult-assoc, need this). Restricted to a bare universal
    # EQUATION (`_is_ueq`): an IMPLICATION goal (a contract like 0<c & a<b ⊢ a*c<b*c) is NOT proved by induction
    # on its var but by gen + discharging the hypotheses (the directed sum-witness rule) -- inducting there is
    # doomed and would burn the budget before the real proof runs. Inner goals keep gen-first.
    if (goal[0] == "all" and _induction_on[0] and _ind_depth[0] == 0
            and ((_is_ueq(goal) and _drives_mult(goal[1])) or _cancel0_shape(goal))):
        ind = _try_natind(sat, goal)
        if ind is not None:
            return ind
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
    # natind (fallback): prove (All P) by Peano INDUCTION when the parametric proof (gen) failed -- the
    # arithmetic case, where the body needs the induction hypothesis. Nesting is capped so the step's own
    # universal can't trigger unbounded re-induction. The `_drives` recursion-position restriction is applied
    # ONLY to MULT-involving goals (where the doomed-induction explosion happens); pure-`+` goals keep the
    # original unrestricted natind, so the additive-lemma proofs are byte-identical (the prover diamond depends
    # on stable lemma shapes -- a `+`-only proof's gamma emission must not drift).
    if (goal[0] == "all" and _induction_on[0] and _ind_depth[0] < _IND_CAP
            and (not _prop_has_m(goal[1]) or _drives(goal[1]))):
        ind = _try_natind(sat, goal)
        if ind is not None:
            return ind
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
    # directed SUM-WITNESS (≤-chaining): goal ∃k.(= (p A k) C). Synthesise a COMPOUND witness from the
    # context's `+`-equality facts, then discharge the body in a FOCUSED sub-proof (only add-assoc, both
    # orientations) -- add-assoc re-associates so the facts rewrite A's sum to C. Pattern-gated, so the
    # general search is untouched. Three witness sources, all real contract obligations:
    #   N-step ≤-transitivity  a≤b≤c…≤z ⊢ a≤z : a path A->…->C in the +slack graph -> witness = sum of slacks
    #   drop-addend            i+k≤n     ⊢ i≤n : fact (p (p A K) M)=C               -> witness K+M
    #   strict <-chaining      i<m≤c     ⊢ i<c : a path whose FIRST edge is STRICT (slack (s D)) -> peel that
    #                                            `s` for the witness; add-succ-left bridges the (s k) goal slot
    #   mult-scaling           a≤b ⊢ a*c≤b*c : goal ∃j.(p (m X c) j)=(m Y c) + fact (p X K)=Y -> witness K*c,
    #                                            proved by right-distributivity (a*c + K*c = (a+K)*c = b*c)
    if (goal[0] == "ex" and _active_lemmas and goal[1][0] == "=" and goal[1][1][0] == "p"):
        body, A, C, slot = goal[1], goal[1][1][1], goal[1][2], goal[1][1][2]
        if slot == ("v", 0):                                       # ≤ goal: ∃k. A+k = C
            wits = []
            path = _slack_path(sat, A, C)
            if path is not None:
                wits.append((_sum_right(path), (_ASSOC,)))
            for p1, _ in sat:
                if (p1[0] == "=" and p1[1][0] == "p" and p1[1][1][0] == "p"
                        and p1[1][1][1] == A and p1[2] == C):       # fact (p (p A K) M)=C
                    wits.append((("p", p1[1][1][2], p1[1][2]), (_ASSOC,)))
            if A[0] == "m" and C[0] == "m" and A[2] == C[2]:        # A=(m X c), C=(m Y c) -- same scale factor c
                for p1, _ in sat:
                    if (p1[0] == "=" and p1[1][0] == "p" and p1[1][1] == A[1] and p1[2] == C[1]):  # fact (p X K)=Y
                        wits.append((("m", p1[1][2], A[2]), (_RDIST,)))
            if A[0] == "p":                                        # ADDITIVE two-bound (non-strict mirror of the
                X2, Y2, cn = A[1], A[2], nf(C)                     # `<` interval below)  a<=b & c<=d ⊢ a+c <= b+d :
                xf = [(f[1][2], f[2]) for f, _ in sat              # goal ∃w.(p (p X Y) w)=C where C is CL+CR. (K, CL)
                      if f[0] == "=" and f[1][0] == "p" and f[1][1] == X2]   # from a<=b (fact X+K=CL); (J, CR) from
                yf = [(f[1][2], f[2]) for f, _ in sat             # c<=d (fact Y+J=CR). witness w = K+J: (X+Y)+(K+J) =
                      if f[0] == "=" and f[1][0] == "p" and f[1][1] == Y2]   # (X+K)+(Y+J) = CL+CR = C (add-assoc +
                for K, CL in xf:                                  # add-comm). This is `add_le_add` -- the sum of two
                    for J, CR in yf:                              # bounded values is bounded by the sum of the bounds.
                        if nf(("p", CL, CR)) == cn:
                            wits.append((("p", K, J), (_ASSOC, _COMM)))
            if C[0] == "p":                                        # ADDITIVE two-bound, LOWER form  LX<=X & LY<=Y ⊢
                XX, YY, an = C[1], C[2], nf(A)                     # LX+LY <= X+Y : goal ∃w. A+w=(p X Y) where A is
                xf = [(f[1][1], f[1][2]) for f, _ in sat          # LX+LY. (LX, K) from LX<=X (fact (p LX K)=X); (LY, J)
                      if f[0] == "=" and f[1][0] == "p" and f[2] == XX]  # from LY<=Y (fact (p LY J)=Y). Dual of the
                yf = [(f[1][1], f[1][2]) for f, _ in sat          # upper form above -- the SUM is the goal's RHS, so
                      if f[0] == "=" and f[1][0] == "p" and f[2] == YY]  # it lower-bounds `LX+LY <= X+Y`. Needed for
                for LX, K in xf:                                  # e.g. 1<=a & 1<=b ⊢ 2<=a+b (a range's lower bound).
                    for LY, J in yf:                              # witness w = K+J: A+(K+J) = (LX+LY)+(K+J) =
                        if nf(("p", LX, LY)) == an:               # (LX+K)+(LY+J) = X+Y (add-assoc + add-comm).
                            wits.append((("p", K, J), (_ASSOC, _COMM)))
        elif slot == ("s", ("v", 0)):                              # < goal: ∃k. A+(s k) = C
            wits = []
            path = _slack_path(sat, A, C)
            if path is not None and path[0][0] == "s":             # first edge strict -> total is strict
                rest = path[1:]
                w = ("p", path[0][1], _sum_right(rest)) if rest else path[0][1]
                wits.append((w, (_ASSOC, _SUCCL)))
            if A[0] == "m" and C[0] == "m" and A[2] == C[2]:       # STRICT mult-scaling  0<c & X<Y ⊢ X*c<Y*c :
                X2, c2, Y2 = A[1], A[2], C[1]                      # goal ∃w.(m X c)+(s w)=(m Y c). From X<Y (fact
                Ks = [f[1][2][1] for f, _ in sat                  # (p X (s K))=Y) and 0<c (fact (p z (s P))=c),
                      if f[0] == "=" and f[1][0] == "p" and f[1][1] == X2 and f[2] == Y2 and f[1][2][0] == "s"]
                Ps = [f[1][2][1] for f, _ in sat                  # witness w = P + K*c, proved by right-distrib:
                      if f[0] == "=" and f[1][0] == "p" and f[1][1] == ("z",) and f[2] == c2 and f[1][2][0] == "s"]
                for K in Ks:                                      # Y*c = (X+(s K))*c = X*c + (s K)*c, and (s K)*c
                    for P in Ps:                                  # = c + K*c = (s P) + K*c = s(P + K*c) since c=s P.
                        wits.append((("p", P, ("m", K, c2)), (_RDIST,)))
            if A[0] == "p":                                        # ADDITIVE two-bound (interval)  X<CL & Y<CR ⊢
                X2, Y2, cn = A[1], A[2], nf(C)                     # X+Y < CL+CR : goal ∃w.(p (p X Y)(s w))=C where
                xf = [(f[1][2][1], f[2]) for f, _ in sat          # C is CL+CR (a symbolic sum OR a concrete numeral).
                      if f[0] == "=" and f[1][0] == "p" and f[1][1] == X2 and f[1][2][0] == "s"]  # (K, CL) from X<CL
                yf = [(f[1][2][1], f[2]) for f, _ in sat
                      if f[0] == "=" and f[1][0] == "p" and f[1][1] == Y2 and f[1][2][0] == "s"]  # (J, CR) from Y<CR
                for K, CL in xf:                                   # witness w = K + (s J): (X+Y)+s(K+s J) = X+Y+(s K)+
                    for J, CR in yf:                              # (s J) = CL+CR (add-assoc + add-comm; then CL+CR
                        if nf(("p", CL, CR)) == cn:                # reduces to C when concrete). "sum of bounded is bounded."
                            wits.append((("p", K, ("s", J)), (_ASSOC, _COMM)))
        else:
            wits = []
        for w, lemma_ids in wits:
            saved, sbo = list(_active_lemmas), _both_orient[0]
            _active_lemmas[:] = [l for l in _active_lemmas if l[3] in lemma_ids]
            _both_orient[0] = True
            pf = prove(sat, subst0(body, w))
            _active_lemmas[:] = saved
            _both_orient[0] = sbo
            if pf is not None:
                return ("wit", body, w, pf)
    # 0 <= C for ANY term C (naturals are non-negative): the goal ∃k. 0+k=C has witness C, since 0+C=C
    # definitionally (refl). The generic witness search below only offers eigenvars + their successors + the
    # goal's ground terms, so it misses a COMPOUND witness like a*a -- this discharges `0<=<anything>`
    # (e.g. 0<=a*a, a squared bound's lower half) directly. Guard on nf so it only fires when it truly closes.
    if (goal[0] == "ex" and goal[1][0] == "=" and goal[1][1] == ("p", ("z",), ("v", 0))
            and _max_free_term(goal[1][2], 0) == 0):    # C is closed w.r.t. the ex-binder -> a valid witness
        C = goal[1][2]
        if nf(("p", ("z",), C)) == nf(C):               # 0 + C = C (always, definitionally) -> witness C, refl
            return ("wit", goal[1], C, ("refl", C))
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
    # disj: an equality hypothesis whose sides reduce to CLASHING constructors (0 vs s _) is absurd -- Peano's
    # zero != successor. (disj e) : bot, then ex falso for any goal. The kernel normalises both sides, so we
    # match on nf too (e.g. (= (p z z) (s z)) clashes).
    for prop, term in sat:
        if prop[0] == "=":
            a, b = nf(prop[1]), nf(prop[2])
            if (a[0] == "z" and b[0] == "s") or (a[0] == "s" and b[0] == "z"):
                return ("disj", term) if goal == ("bot",) else ("absurd", goal, ("disj", term))
    # ORDER-CYCLE refutation (the one bit of FORWARD order reasoning): a strict cycle in context -- two facts
    # (p A (s _))=B and (p B (s _))=A, i.e. A<B and B<A -- is absurd. The sum-witness already CHAINS the cycle
    # into (Lt A A) as a GOAL; the banked irreflexivity ∀a.a<a->⊥ then refutes it. Fires ONLY on a structural
    # cycle (cheap scan, no cost when absent) and only in phase 2 (irreflexivity needs induction). This reaches
    # lt-asymmetry -- the goal-directed rules alone can't, since combining two order facts is forward reasoning.
    if goal == ("bot",) and _induction_on[0]:
        for p1, _ in sat:
            ab = _strict_base(p1)
            if ab is None or ab[0] == ab[1]:   # skip a TRIVIAL self-cycle (A<A): a<a->⊥ is proven directly by
                continue                        # induction (this rule would recurse into its own irreflexivity)
            for p2, _ in sat:
                ba = _strict_base(p2)
                if ba is not None and ba[0] == ab[1] and ba[1] == ab[0]:   # A<B and B<A -> non-trivial cycle at A
                    base = ab[0]
                    pf_lt = prove(sat, ("ex", ("=", ("p", base, ("s", ("v", 0))), base)))   # (Lt base base)
                    if pf_lt is not None:
                        irr = prove([], _IRREFL)   # irreflexivity is CLOSED -- prove from a clean context so the
                        if irr is not None:        # cycle facts here don't re-trigger this rule (non-termination)
                            return ("app", ("inst", irr, base), pf_lt)
    # ORDER-EQ refutation (ne_of_lt): a strict fact A<B together with an equality A=B (or B=A) is absurd --
    # the equality collapses A<B into A<A, which the banked irreflexivity refutes. This is the eq-and-strict
    # combination the plain ORDER-CYCLE (two strict facts) can't see; it discharges distinct_when_ordered
    # (∀i∀j. i<j -> i!=j, i.e. i<j -> i=j -> ⊥). Like ORDER-CYCLE it fires only for ⊥ goals in phase 2
    # (irreflexivity needs induction) and builds every cert via a recursive prove(): the (Lt A A) sub-goal
    # closes because the strict fact's witness equation A+(s k)=B rewrites B->A through the equality.
    if goal == ("bot",) and _induction_on[0]:
        for p1, _ in sat:
            ab = _strict_base(p1)
            if ab is None or ab[0] == ab[1]:   # a self-strict A<A already refutes via irreflexivity directly
                continue
            A, B = ab
            for p2, _ in sat:
                if p2[0] == "=" and ((p2[1] == A and p2[2] == B) or (p2[1] == B and p2[2] == A)):
                    pf_lt = prove(sat, ("ex", ("=", ("p", A, ("s", ("v", 0))), A)))   # (Lt A A)
                    if pf_lt is not None:
                        irr = prove([], _IRREFL)   # closed context: the strict/eq facts here don't re-fire this rule
                        if irr is not None:
                            return ("app", ("inst", irr, A), pf_lt)
    # le-ANTISYMMETRY: goal a=b with an additive cycle a+k=b, b+j=a (a<=b and b<=a) in context. Derive a=b via
    #   a+(k+j)=a [chain, both-orient add-assoc] -> k+j=0 [CANCEL0] -> k=0 [positivity]; then a=b closes by the
    #   existing rewrite (b->a+k, k->0, add-0). Forward orchestration; every cert built by a recursive prove().
    #   Fires only on a genuine cycle (cheap otherwise) and once (the k=0 guard blocks re-entry -> terminates).
    if (goal[0] == "=" and _induction_on[0]
            and goal[1][0] == "eig" and goal[2][0] == "eig"):   # a=b with BARE eigenvar sides -- the antisym
        for p1, _ in sat:                                        # shape; fuzz/sum-witness `=` goals have compound
            f1 = _add_base(p1)                                   # sides, so the expensive cycle search is skipped
            if f1 is None or f1[0] == f1[2]:
                continue
            A, K, B = f1
            if set([A, B]) != set([goal[1], goal[2]]) or has(sat, ("=", K, ("z",))) is not None:
                continue
            for p2, _ in sat:
                f2 = _add_base(p2)
                if f2 is not None and f2[0] == B and f2[2] == A:
                    sbo = _both_orient[0]; _both_orient[0] = True
                    cyc = prove(sat, ("=", ("p", A, ("p", K, f2[1])), A))   # a+(k+j)=a
                    _both_orient[0] = sbo
                    if cyc is None:
                        continue
                    cpf, ppf = prove([], _CANCEL0), prove([], _POSIT)
                    if cpf is None or ppf is None:
                        continue
                    kj0 = ("app", ("inst", ("inst", cpf, A), ("p", K, f2[1])), cyc)   # k+j=0
                    k0 = ("app", ("inst", ("inst", ppf, K), f2[1]), kj0)              # k=0
                    body = prove(sat + [(("=", K, ("z",)), k0)], goal)
                    if body is not None:
                        return body
    # le-CANCEL-RIGHT: goal a<=b = ∃m.a+m=b, context has (a+c)+k=b+c (a+c<=b+c). Witness m=k; prove the body
    #   a+k=b by rearranging (a+k)+c = (a+c)+k = b+c [general AC search + the fact] then cancelling c
    #   (add-cancel-right). A witness source the directed sum-witness can't reach (its fact's RHS is b+c, not b).
    #   Placed AFTER the sum-witness, so it fires only when that misses AND the specific (A+C)+K=B+C fact exists.
    if (goal[0] == "ex" and goal[1][0] == "=" and goal[1][1][0] == "p"
            and goal[1][1][2] == ("v", 0)):
        A, B = goal[1][1][1], goal[1][2]
        for f, _ in sat:
            if (f[0] == "=" and f[1][0] == "p" and f[1][1][0] == "p" and f[1][1][1] == A
                    and f[2][0] == "p" and f[2][1] == B and f[1][1][2] == f[2][2]):   # (A+C)+K = B+C
                C, K = f[1][1][2], f[1][2]
                comb = prove(sat, ("=", ("p", ("p", A, K), C), ("p", B, C)))   # (a+k)+c = b+c
                if comb is None:
                    continue
                acr = prove([], _ACR)
                if acr is None:
                    continue
                bpf = ("app", ("inst", ("inst", ("inst", acr, ("p", A, K)), B), C), comb)  # a+k=b
                return ("wit", goal[1], K, bpf)
    # sinj: successor injectivity -- a hypothesis (s a = s b) yields the NEW fact (a = b). The children are
    # the normal forms' successors' arguments (what the kernel's sinj returns). Decreasing successor depth +
    # the new-fact guard terminate the chain (so 2=3 collapses 2=3 -> 1=2 -> 0=1 -> disj -> bot).
    for prop, term in sat:
        if prop[0] == "=":
            a, b = nf(prop[1]), nf(prop[2])
            if a[0] == "s" and b[0] == "s":
                fact = ("=", a[1], b[1])
                if has(sat, fact) is None:
                    body = prove(sat + [(fact, ("sinj", term))], goal)
                    if body is not None:
                        return body
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
    # goal-normalisation: if the goal reduces, prove its normal form (definitionally equal, so a proof of
    # nf_prop(goal) is accepted for goal by the kernel's conversion -- returned as-is). This runs BEFORE the
    # rewrite rule so a reducible successor like (p (s n) z) is first exposed as (s (p n z)); otherwise the
    # rewrite rule fires on the raw goal in the GROWING direction and spirals. This is the key that closes the
    # natind step (the IH then rewrites (p n z) -> n on the normalised goal).
    ng = nf_prop(goal)
    if ng != goal:
        pf = prove(sat, ng)
        if pf is not None:
            return pf
    # L-eqrewrite: rewrite the goal with an equality hypothesis e:(= x y), via eqelim (Leibniz transport),
    # in BOTH orientations -- so symmetry, transitivity, congruence and transport all reduce to this one rule.
    # Orientation 1 rewrites y->x directly; orientation 2 rewrites x->y through the derived symmetric proof
    # sym(e) = (eqelim (= (v0) x) e (refl x)) : (= y x). Each only fires when the side actually occurs (so the
    # subgoal differs from the goal -> progress), and only on GROUND sides (no binder capture). The memo on
    # the goal cuts the a<->b ping-pong, so it terminates.
    if goal[0] in ("pred", "rel", "=", "->", "&", "+", "all", "ex"):
        # Collect every applicable rewrite, then try the ones that SHRINK the goal first. Preferring the
        # smaller result keeps the search on the productive (toward-refl) direction -- the growing direction
        # (e.g. n -> (p n z)) otherwise spirals before the shrinking one is ever reached. The absolute cap
        # bounds the rare case where a rewrite must grow (transitivity through a larger middle term).
        rewrites = []
        for prop, term in sat:
            if prop[0] != "=":
                continue
            x, y = prop[1], prop[2]
            if _ground(y) and occurs_prop(goal, y):       # orient 1: e:(= x y) rewrites y -> x
                mot = abstract_prop(goal, y)
                sub = subst0(mot, x)
                if sub != goal:
                    rewrites.append((prop_size(sub), mot, sub, term))
            if _ground(x) and occurs_prop(goal, x):       # orient 2: sym(e):(= y x) rewrites x -> y
                mot = abstract_prop(goal, x)
                sub = subst0(mot, y)
                if sub != goal:
                    syme = ("eqelim", ("=", ("v", 0), x), term, ("refl", x))
                    rewrites.append((prop_size(sub), mot, sub, syme))
        # library-lemma rewrites: match a lemma's LHS (and, when _both_orient, its RHS) against a goal subterm
        # -> a directed rewrite, the equation proved by citing the lemma (use idx) instantiated at σ. Folded
        # into the shrink-first list (not as context facts) so the search stays bounded. Both-orientation is
        # gated: OFF in the general search (it bloats it), ON only in the directed sum-chain rule's sub-proof.
        for arity, lhs, rhs, idx in _active_lemmas:
            dirs = ((lhs, rhs, True), (rhs, lhs, False)) if _both_orient[0] else ((lhs, rhs, True),)
            for pat, other, flip in dirs:
                for sub_t in _prop_subterms(goal):
                    s = _match_term(pat, sub_t, {})
                    if s is None or len(s) != arity:
                        continue
                    frm, to = _fill(pat, s), _fill(other, s)
                    if frm == to:
                        continue
                    if not _ground(frm):              # frm sits under a binder (contains a bound de Bruijn var,
                        continue                      # e.g. the ∃-bound k in (p a (s (v 0)))) -> abstracting +
                    mot = abstract_prop(goal, frm)    # instantiating it captures the binder and emits a cert the
                    sub = subst0(mot, to)             # kernel rejects. Same GROUND guard the `=` rewrite has.
                    if sub == goal:
                        continue
                    pe = ("use", idx)
                    for j in range(arity - 1, -1, -1):  # instantiate outermost binder first -> pe : (= Lσ Rσ)
                        pe = ("inst", pe, s[j])
                    # eqelim needs pf_eq : (= to frm). Matching the LHS gives pe : (= frm to) -> wrap in sym;
                    # matching the RHS gives pe : (= to frm) already.
                    if flip:
                        pe = ("eqelim", ("=", ("v", 0), frm), pe, ("refl", frm))
                    rewrites.append((prop_size(sub), mot, sub, pe))
        # local UNIVERSAL-EQUATION hypotheses as directed rewrites -- the same mechanism as the library
        # lemmas, but the cited proof is `inst…(the hypothesis)` rather than `(use idx)`. The INDUCTION
        # HYPOTHESIS is exactly such a hypothesis: a natind step over (All x… (= L R)) adds the IH
        # (All x… (= L R))[n] to context, and to close the step the IH must REWRITE the step goal (e.g.
        # left-distributivity's IH a*(x+y)=a*x+a*y rewrites (s a)*(x+y)'s reduct into the interchange the
        # library then closes -- the precise unlock for mult-assoc). The ground-`=` hypothesis rewrite
        # above never fires here (the IH's top is `all`, not `=`). Gated to phase 2 (induction active),
        # like the library, so the general search is untouched.
        if _induction_on[0]:
            for prop, hyp in sat:
                peeled = _peel(prop)
                if peeled is None or peeled[0] == 0:
                    continue
                arity, lhs, rhs = peeled
                dirs = ((lhs, rhs, True), (rhs, lhs, False)) if _both_orient[0] else ((lhs, rhs, True),)
                for pat, other, flip in dirs:
                    for sub_t in _prop_subterms(goal):
                        s = _match_term(pat, sub_t, {})
                        if s is None or len(s) != arity:
                            continue
                        frm, to = _fill(pat, s), _fill(other, s)
                        if frm == to:
                            continue
                        if not _ground(frm):          # no binder capture (see the library-lemma rewrite above)
                            continue
                        mot = abstract_prop(goal, frm)
                        sub = subst0(mot, to)
                        if sub == goal:
                            continue
                        pe = hyp
                        for j in range(arity - 1, -1, -1):  # instantiate outermost binder first
                            pe = ("inst", pe, s[j])
                        if flip:
                            pe = ("eqelim", ("=", ("v", 0), frm), pe, ("refl", frm))
                        rewrites.append((prop_size(sub), mot, sub, pe))
        rewrites.sort(key=lambda r: r[0])
        for sz, mot, sub, pe in rewrites:
            if sz <= _rw_cap[0]:
                pf = prove(sat, sub)
                if pf is not None:
                    return ("eqelim", mot, pe, pf)
    return None


def to_db(term, binders, ib=()):  # convert the named proof term to check.beta's de Bruijn syntax
    h = term[0]                    # `binders` = hypothesis names; `ib` = eigenvar ids (individual binders)
    if h == "hyp":
        return "(hyp %d)" % (len(binders) - 1 - binders.index(term[1]))
    if h == "use":  # cite a library lemma proved in the certificate's def-prelude
        return "(use %d)" % term[1]
    if h == "lam":
        _, nm, prop, body = term
        return "(lam %s %s)" % (beta_prop(prop, ib), to_db(body, binders + [nm], ib))
    if h in ("fst", "snd"):
        return "(%s %s)" % (h, to_db(term[1], binders, ib))
    if h in ("inl", "inr", "absurd"):  # carry a PROP annotation, then a proof
        return "(%s %s %s)" % (h, beta_prop(term[1], ib), to_db(term[2], binders, ib))
    if h == "case":  # scrutinee + two lam branches
        return "(case %s %s %s)" % (to_db(term[1], binders, ib), to_db(term[2], binders, ib), to_db(term[3], binders, ib))
    if h == "refl":  # reflexivity of equality: (refl t) : (= t t), accepted up to the kernel's conversion
        return "(refl %s)" % beta_term(term[1], ib)
    if h == "disj":  # zero != successor: (disj pf_eq) : bot, from a clashing equality proof
        return "(disj %s)" % to_db(term[1], binders, ib)
    if h == "sinj":  # successor injectivity: (sinj pf_eq) : (= a b), from (= (s a) (s b))
        return "(sinj %s)" % to_db(term[1], binders, ib)
    if h == "eqelim":  # Leibniz transport: (eqelim motive pf_eq pf_pa) -- the motive's rewrite-hole is an
        _, mot, pe, pa = term  # IMPLICIT de Bruijn-0 binder, so its free terms (e.g. an eigenvar) emit at depth 1
        return "(eqelim %s %s %s)" % (beta_prop(mot, ib, 1), to_db(pe, binders, ib), to_db(pa, binders, ib))
    if h == "gen":  # universal introduction: push the eigenvar -> a new innermost individual binder
        _, e, body = term
        return "(gen %s)" % to_db(body, binders, tuple(ib) + (e,))
    if h == "natind":  # Peano induction: motive (implicit binder -> depth 1), base : P(0), step : P(n)->P(s n)
        _, motive, base, step = term
        return "(natind %s %s %s)" % (beta_prop(motive, ib, 1), to_db(base, binders, ib), to_db(step, binders, ib))
    if h == "inst":  # universal elimination: a proof applied to a TERM
        return "(inst %s %s)" % (to_db(term[1], binders, ib), beta_term(term[2], ib))
    if h == "wit":  # existential introduction: body-prop, witness TERM, proof of the instance. The body has
        # an IMPLICIT binder (the existential slot at v0), so its free vars sit one level up -> emit at depth 1
        # (unpack needs no such bump: its slot is an eigenvar already carried in `ib`).
        return "(wit %s %s %s)" % (beta_prop(term[1], ib, 1), beta_term(term[2], ib), to_db(term[3], binders, ib))
    if h == "unpack":  # existential elimination: the handler is `(gen (lam body C))` -- one individual
        _, exterm, body, e, nm, pf = term   # binder (the witness) + one hypothesis binder (the body)
        ib2 = tuple(ib) + (e,)
        handler = "(gen (lam %s %s))" % (beta_prop(body, ib2), to_db(pf, binders + [nm], ib2))
        return "(unpack %s %s)" % (to_db(exterm, binders, ib), handler)
    return "(%s %s %s)" % (h, to_db(term[1], binders, ib), to_db(term[2], binders, ib))  # pair / app


def emit_cert(goal, proof):  # the full certificate: the lemma def-prelude (if any), then the goal + its proof
    prelude = "".join("(def %d %s %s) " % (i, beta_prop(p), to_db(lpf, [], ()))
                      for i, (p, lpf) in enumerate(_used_lemmas[0]))
    return "%s%s %s" % (prelude, beta_prop(goal), to_db(proof, [], tuple(_base_ib)))


def _shift_uses(t, off):  # shift every (use i) citation in a proof term to (use i+off) -- for relocating a
    if not isinstance(t, tuple):  # def/use lemma block to a later id range in a shared library.
        return t
    if t and t[0] == "use":
        return ("use", t[1] + off)
    return tuple(_shift_uses(x, off) for x in t)


def emit_lib_block(goal, proof, offset):  # a SELF-CONTAINED def/use library block for banking a big derived
    # lemma without inlining: emit its base lemmas as `(def offset+i ..)` and the lemma itself as the LAST def,
    # all (use i) shifted by `offset`. Unlike emit_inline (which duplicates each cited lemma at every use site,
    # exploding a reuse-heavy proof past the checker's arena), this shares the bases -- so a proof that cites
    # add-comm 270+ times stays ~60 KB, not ~235 KB. The block is self-contained: the enclosing library only
    # cites the final def (the lemma), never the block's internal bases, so their arrangement is private.
    out = []
    for i, (p, lpf) in enumerate(_used_lemmas[0]):
        out.append("(def %d %s %s)" % (offset + i, beta_prop(p), to_db(_shift_uses(lpf, offset), [], ())))
    lemma_id = offset + len(_used_lemmas[0])
    out.append("(def %d %s %s)" % (lemma_id, beta_prop(goal), to_db(_shift_uses(proof, offset), [], tuple(_base_ib))))
    return "".join(out), lemma_id


# ---- A SECOND emission target: checker.gamma's input syntax (algebraic-data constructors, run on the gamma
# reference interpreter). The PROVER DIAMOND runs each emitted cert through BOTH check.beta and checker.gamma
# -- two independently-written checkers at different rungs -- so the prover's actual cert shapes (not just
# random fuzzed proofs) must pass both. A syntactic mirror of beta_term/beta_prop/to_db; same de Bruijn, only
# the constructor names differ (and propositional atoms become integers). The lemma `(def N)/(use N)` prelude
# has no gamma analogue, so lemma-using certs are reported "unsupported" and skipped by the diamond. ----
def _atom_id(name, atoms):
    if name not in atoms:
        atoms[name] = len(atoms)
    return atoms[name]


def gamma_term(t, ib=(), depth=0):
    if t[0] == "z":
        return "Ze"
    if t[0] == "s":
        return "(Su %s)" % gamma_term(t[1], ib, depth)
    if t[0] in ("p", "m"):
        return "(%s %s %s)" % ("Pl" if t[0] == "p" else "Mu", gamma_term(t[1], ib, depth), gamma_term(t[2], ib, depth))
    if t[0] == "eig":
        return "(Iv %d)" % (depth + len(ib) - 1 - list(ib).index(t[1]))
    return "(Iv %d)" % t[1]


def gamma_prop(p, atoms, ib=(), depth=0):
    h = p[0]
    if h == "at":
        return "(Atom %d)" % _atom_id(p[1], atoms)
    if h == "bot":
        return "Bot"
    if h == "pred":
        return "(Pred %d %s)" % (p[1], gamma_term(p[2], ib, depth))
    if h == "rel":
        return "(Rel %d %s %s)" % (p[1], gamma_term(p[2], ib, depth), gamma_term(p[3], ib, depth))
    if h == "=":
        return "(Eq %s %s)" % (gamma_term(p[1], ib, depth), gamma_term(p[2], ib, depth))
    if h in ("all", "ex"):
        return "(%s %s)" % ("All" if h == "all" else "Exists", gamma_prop(p[1], atoms, ib, depth + 1))
    return "(%s %s %s)" % ({"->": "Arrow", "&": "And", "+": "Or"}[h],
                           gamma_prop(p[1], atoms, ib, depth), gamma_prop(p[2], atoms, ib, depth))


def to_gamma(term, binders, atoms, ib=()):
    h = term[0]
    if h == "hyp":
        return "(Hyp %d)" % (len(binders) - 1 - binders.index(term[1]))
    if h == "lam":
        _, nm, prop, body = term
        return "(Lam %s %s)" % (gamma_prop(prop, atoms, ib), to_gamma(body, binders + [nm], atoms, ib))
    if h in ("fst", "snd"):
        return "(%s %s)" % (h.capitalize(), to_gamma(term[1], binders, atoms, ib))
    if h in ("inl", "inr"):
        return "(%s %s %s)" % (h.capitalize(), gamma_prop(term[1], atoms, ib), to_gamma(term[2], binders, atoms, ib))
    if h == "absurd":
        return "(Absurd %s %s)" % (gamma_prop(term[1], atoms, ib), to_gamma(term[2], binders, atoms, ib))
    if h == "case":
        return "(Case %s %s %s)" % (to_gamma(term[1], binders, atoms, ib), to_gamma(term[2], binders, atoms, ib), to_gamma(term[3], binders, atoms, ib))
    if h == "refl":
        return "(Refl %s)" % gamma_term(term[1], ib)
    if h in ("disj", "sinj"):
        return "(%s %s)" % (h.capitalize(), to_gamma(term[1], binders, atoms, ib))
    if h == "gen":
        _, e, body = term
        return "(Gen %s)" % to_gamma(body, binders, atoms, tuple(ib) + (e,))
    if h == "natind":  # Peano induction: motive carries an implicit binder -> depth 1, like the eqelim motive
        _, motive, base, step = term
        return "(Natind %s %s %s)" % (gamma_prop(motive, atoms, ib, 1), to_gamma(base, binders, atoms, ib), to_gamma(step, binders, atoms, ib))
    if h == "inst":
        return "(Inst %s %s)" % (to_gamma(term[1], binders, atoms, ib), gamma_term(term[2], ib))
    if h == "wit":
        return "(Wit %s %s %s)" % (gamma_prop(term[1], atoms, ib, 1), gamma_term(term[2], ib), to_gamma(term[3], binders, atoms, ib))
    if h == "eqelim":
        _, mot, pe, pa = term
        return "(Eqelim %s %s %s)" % (gamma_prop(mot, atoms, ib, 1), to_gamma(pe, binders, atoms, ib), to_gamma(pa, binders, atoms, ib))
    if h == "unpack":
        _, exterm, body, e, nm, pf = term
        ib2 = tuple(ib) + (e,)
        return "(Unpack %s (Gen (Lam %s %s)))" % (to_gamma(exterm, binders, atoms, ib), gamma_prop(body, atoms, ib2), to_gamma(pf, binders + [nm], atoms, ib2))
    if h == "use":
        raise ValueError("lemma `use` has no gamma-checker analogue")
    return "(%s %s %s)" % ({"pair": "Pair", "app": "App"}[h], to_gamma(term[1], binders, atoms, ib), to_gamma(term[2], binders, atoms, ib))


def _rename_eigs(t, off):  # shift every eigenvariable id in a proof term (and its embedded props/terms) by
    if not isinstance(t, tuple):   # `off`. Eigenvar ids appear as ("eig", e) terms and as the BARE id in
        return t                    # (gen e body) and (unpack exterm body e nm pf); atom ids / hyp names are
    if t[0] == "eig":               # left alone. de Bruijn is recovered from the eig STACK at emit time, so a
        return ("eig", t[1] + off)  # SHIFT preserves the proof's meaning while making its eigenvars disjoint.
    if t[0] == "gen":
        return ("gen", t[1] + off, _rename_eigs(t[2], off))
    if t[0] == "unpack":
        return ("unpack", _rename_eigs(t[1], off), _rename_eigs(t[2], off), t[3] + off, t[4], _rename_eigs(t[5], off))
    return tuple(_rename_eigs(x, off) for x in t)


def _inline_uses(t, lemma_proofs):  # splice each (use i) with lemma i's (closed) proof, recursively
    if not isinstance(t, tuple):
        return t
    if t and t[0] == "use":
        return _inline_uses(lemma_proofs[t[1]], lemma_proofs)
    return tuple(_inline_uses(x, lemma_proofs) for x in t)


_EIG_OFF = 100000  # per-lemma eigenvar offset for inlining: lemma i's eigenvars shift to (i+1)*_EIG_OFF so they
# can't COLLIDE with the goal proof's (which start at 0 every solve, since _setup resets the counter) or with
# each other. Without this, splicing a lemma whose eigenvar id matches a goal eigenvar mis-resolves de Bruijn.


def _inline_offset(proof):  # inline the def-prelude into `proof`, first making each lemma's eigenvars disjoint
    if not _used_lemmas[0]:
        return proof
    lemmas = [_rename_eigs(pf, (i + 1) * _EIG_OFF) for i, (_, pf) in enumerate(_used_lemmas[0])]
    return _inline_uses(proof, lemmas)


def emit_inline(goal, proof):  # a SELF-CONTAINED check.beta proof: each (use i) is inlined with lemma i's
    # CLOSED proof, so the proof needs NO def-prelude and can stand alone as one `(def N prop proof)` at a
    # STABLE id. Same splice as emit_gamma (sound: lemma proofs are closed -> de Bruijn stays correct), but
    # emitted in check.beta syntax. Lets the prover GENERATE the contract lemma library (one inlined def per
    # lemma) that discharge.rs cites -- the hand-written .proof base becomes automation output, kernel-checked.
    proof = _inline_offset(proof)
    return "%s\t%s" % (beta_prop(goal), to_db(proof, [], tuple(_base_ib)))


def emit_gamma(goal, proof):  # `(check PROOF GOAL)` for checker.gamma -- a single proof (no def/use prelude,
    # which gamma lacks). Lemma citations are INLINED: (use i) -> lemma i's proof. Sound because each lemma
    # proof is CLOSED (references only its own binders), so splicing it anywhere keeps the de Bruijn correct.
    proof = _inline_offset(proof)
    atoms = {}
    g = gamma_prop(goal, atoms)                  # goal first, so its atoms get stable ids
    p = to_gamma(proof, [], atoms, tuple(_base_ib))
    return "(check %s %s)" % (p, g)


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
            print("%s\t%s" % (beta_prop(goal), emit_cert(goal, proof)))


# ---- FIRST-ORDER fuzz: stress the eigenvariable / de Bruijn emission in gen/inst/wit/unpack. Random
# first-order goals are almost never tautologies (so almost no certs would be emitted to check), so instead
# we sample from PROVABLE SCHEMAS with random predicate/term fillings -- each is valid by construction, so
# the prover should discharge it and, crucially, the kernel must ACCEPT the emitted certificate. A de Bruijn
# slip in eigenvar emission would surface as a kernel REJECT here. ----
def _rterm(rng, depth, nvars):  # a term valid under `nvars` enclosing quantifier binders
    r = rng.random()
    if nvars > 0 and r < 0.5:
        return ("v", rng.randrange(nvars))
    if depth > 0 and r < 0.75:
        return ("s", _rterm(rng, depth - 1, nvars))
    return ("z",)


def _ratom(rng, nvars):  # an atomic predicate/relation over terms valid under nvars binders
    if rng.random() < 0.6:
        return ("pred", rng.randrange(3), _rterm(rng, 2, nvars))
    return ("rel", rng.randrange(2), _rterm(rng, 2, nvars), _rterm(rng, 2, nvars))


def _rbody(rng, d, nvars):  # a propositional body (no quantifiers) over atoms valid under nvars binders
    if d <= 0 or rng.random() < 0.5:
        return _ratom(rng, nvars)
    return (rng.choice(("&", "->", "+")), _rbody(rng, d - 1, nvars), _rbody(rng, d - 1, nvars))


def random_foprop(rng):  # a first-order goal that is PROVABLE by construction (random-filled schema)
    s = rng.randrange(6)
    if s == 0:                                            # ∀x. (B -> B)            -- gen + ->-intro
        b = _rbody(rng, 2, 1)
        return ("all", ("->", b, b))
    if s == 1:                                            # ∀x.∀y. (B -> B)         -- nested gen
        b = _rbody(rng, 2, 2)
        return ("all", ("all", ("->", b, b)))
    if s == 2:                                            # B[g] -> ∃x.B            -- wit
        b = _rbody(rng, 2, 1)
        g = _rterm(rng, 2, 0)
        return ("->", subst0(b, g), ("ex", b))
    if s == 3:                                            # (∀x.B) -> B[g]          -- inst
        b = _rbody(rng, 2, 1)
        g = _rterm(rng, 2, 0)
        return ("->", ("all", b), subst0(b, g))
    if s == 4:                                            # (∃x.(B&C)) -> ∃x.B      -- unpack + fst + wit
        b = _rbody(rng, 1, 1)
        c = _rbody(rng, 1, 1)
        return ("->", ("ex", ("&", b, c)), ("ex", b))
    # (∃x.B & ∀x.(B->C)) -> ∃x.C                          -- unpack + inst + MP + wit (existential instantiation)
    b = _rbody(rng, 1, 1)
    c = _rbody(rng, 1, 1)
    return ("->", ("&", ("ex", b), ("all", ("->", b, c))), ("ex", c))


def fobatch(n, seed):
    # n provable-by-construction first-order goals; for every cert the prover emits, print it for check.beta.
    import random
    rng = random.Random(seed)
    for _ in range(n):
        goal = random_foprop(rng)
        proof = solve(goal)
        if proof is not None:
            print("%s\t%s" % (beta_prop(goal), emit_cert(goal, proof)))


# ---- ARITHMETIC fuzz: validates that nf() matches check.beta's `normalize` EXACTLY. Build a random closed
# term over z/s/p/m, compute its value in Python, and assert (= term <numeral>). It is true by construction,
# so the prover must discharge it via refl AND the kernel must accept -- which holds iff nf agrees with the
# kernel's reduction. A divergence (a botched plus/mult rule, an off-by-one) surfaces as a kernel REJECT. ----
def _numeral(n):
    t = ("z",)
    for _ in range(n):
        t = ("s", t)
    return t


def _rarith(rng, depth):  # a random closed arithmetic term paired with its integer value
    if depth <= 0 or rng.random() < 0.45:
        n = rng.randrange(4)
        return _numeral(n), n
    op = rng.choice(("s", "p", "m"))
    if op == "s":
        a, va = _rarith(rng, depth - 1)
        return ("s", a), va + 1
    a, va = _rarith(rng, depth - 1)
    b, vb = _rarith(rng, depth - 1)
    if op == "p":
        return ("p", a, b), va + vb
    return ("m", a, b), va * vb


def arithbatch(n, seed):
    import random
    rng = random.Random(seed)
    for _ in range(n):
        t, v = _rarith(rng, rng.randint(1, 3))
        goal = ("=", t, _numeral(v))
        proof = solve(goal)
        if proof is not None:
            print("%s\t%s" % (beta_prop(goal), emit_cert(goal, proof)))


# ---- INEQUALITY fuzz: provable-by-construction CONTRACT-DISCHARGE bound goals (transitivity, drop-addend,
# weakenings) with random successor-nested fillings -- hardens the directed sum-witness + lemma + eqelim-chain
# emission (the riskiest recent de Bruijn). Every emitted cert must kernel-accept; a slip surfaces as a REJECT.
def _ineq_term(rng, base):  # "(v base)" wrapped in 0-2 successors
    t = "(v %d)" % base
    for _ in range(rng.randrange(3)):
        t = "(s %s)" % t
    return t


def random_ineq_str(rng):
    a, b, c = _ineq_term(rng, 0), _ineq_term(rng, 1), _ineq_term(rng, 2)
    s = rng.randrange(5)
    if s == 0:                                                        # a<=b & b<=c -> a<=c  (transitivity)
        return "(-> (& (Le %s %s) (Le %s %s)) (Le %s %s))" % (a, b, b, c, a, c)
    if s == 1:                                                        # i+k<=n -> i<=n        (drop-addend)
        return "(-> (Le (p %s %s) %s) (Le %s %s))" % (a, b, c, a, c)
    if s == 2:                                                        # x<y -> x<=y           (weakening)
        return "(-> (Lt %s %s) (Le %s %s))" % (a, b, a, b)
    if s == 3:                                                        # x<y -> x+1<=y
        return "(-> (Lt %s %s) (Le (s %s) %s))" % (a, b, a, b)
    return "(-> (Le %s %s) (Le %s (s %s)))" % (a, b, a, b)            # x<=y -> x<=y+1


def ineqbatch(n, seed):
    import random
    rng = random.Random(seed)
    for _ in range(n):
        goal = parse(tokenize(random_ineq_str(rng)))
        proof = solve(goal)
        if proof is not None:
            print("%s\t%s" % (beta_prop(goal), emit_cert(goal, proof)))


def main():
    if sys.argv[1] == "--gen":
        gen(int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 1)
        return
    if sys.argv[1] == "--batch":
        batch(int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 1, 16)
        return
    if sys.argv[1] == "--fobatch":  # first-order provable-schema fuzz (exercises eigenvar emission)
        fobatch(int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 1)
        return
    if sys.argv[1] == "--arithbatch":  # closed-arithmetic equality fuzz (validates nf vs the kernel)
        arithbatch(int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 1)
        return
    if sys.argv[1] == "--ineqbatch":  # contract-discharge bound fuzz (directed sum-witness + lemma emission)
        ineqbatch(int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 1)
        return
    if sys.argv[1] == "--gamma":  # emit the cert in checker.gamma syntax (for the prover diamond)
        goal = parse(tokenize(sys.argv[2]))
        proof = solve(goal)
        print("unprovable" if proof is None else emit_gamma(goal, proof))
        return
    if sys.argv[1] == "--inline":  # emit a self-contained beta proof (uses inlined): "<prop>\t<proof>". For
        goal = parse(tokenize(sys.argv[2]))   # GENERATING a contract lemma library (one inlined def per lemma).
        proof = solve(goal)
        print("unprovable" if proof is None else emit_inline(goal, proof))
        return
    if sys.argv[1] == "--libblock":  # emit a def/use library block at an id OFFSET: "<block>\t<lemma id>". For
        goal = parse(tokenize(sys.argv[2]))   # banking a big derived lemma that is too large to inline.
        offset = int(sys.argv[3])
        proof = solve(goal)
        if proof is None:
            print("unprovable")
        else:
            block, lid = emit_lib_block(goal, proof, offset)
            print("%s\t%d" % (block, lid))
        return
    goal = parse(tokenize(sys.argv[1]))
    proof = solve(goal, int(sys.argv[2]) if len(sys.argv) > 2 else 16)
    if proof is None:
        print("unprovable")
    else:
        print(emit_cert(goal, proof))


if __name__ == "__main__":
    main()
