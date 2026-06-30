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

# ---- parse a propositional goal (uppercase atoms, `->`, `&`, parenthesised) into a tuple tree ----
def tokenize(s):
    return s.replace("(", " ( ").replace(")", " ) ").split()


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
        raise ValueError("bad prop head: %s" % head)
    return ("at", t)  # an atom name (a bare uppercase ident)


def beta_prop(p):
    if p[0] == "at":
        return p[1]
    if p[0] == "bot":
        return "(bot)"
    return "(%s %s %s)" % (p[0], beta_prop(p[1]), beta_prop(p[2]))


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
_memo = {}


def solve(goal, _fuel=None):
    _budget[0] = 200000
    _memo.clear()
    return prove([], goal)


def prove(ctx, goal):
    if _budget[0] <= 0:
        return None
    _budget[0] -= 1
    sat = saturate(ctx)
    key = (frozenset(p for p, _ in sat), goal)
    if key in _memo:  # already failed, or in progress (a cycle): no proof to be found this way
        return None
    _memo[key] = None  # tentatively mark unprovable (loop-break); cleared on success below
    proof = _rules(sat, goal)
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
    # absurd: a falsity in context proves anything (ex falso quodlibet)
    bot = has(sat, ("bot",))
    if bot is not None and goal != ("bot",):
        return ("absurd", goal, bot)
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


def to_db(term, binders):  # convert named-hyp proof term to check.beta's de Bruijn `(hyp i)` syntax
    h = term[0]
    if h == "hyp":
        return "(hyp %d)" % (len(binders) - 1 - binders.index(term[1]))
    if h == "lam":
        _, nm, prop, body = term
        return "(lam %s %s)" % (beta_prop(prop), to_db(body, binders + [nm]))
    if h in ("fst", "snd"):
        return "(%s %s)" % (h, to_db(term[1], binders))
    if h in ("inl", "inr", "absurd"):  # carry a PROP annotation, then a proof
        return "(%s %s %s)" % (h, beta_prop(term[1]), to_db(term[2], binders))
    if h == "case":  # scrutinee + two lam branches
        return "(case %s %s %s)" % (to_db(term[1], binders), to_db(term[2], binders), to_db(term[3], binders))
    return "(%s %s %s)" % (h, to_db(term[1], binders), to_db(term[2], binders))  # pair / app


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
