#!/usr/bin/env python3
# A PROOF-SEARCH FRONT LINE -- the Omega pattern in miniature (rungs/omega.md): untrusted automation
# discharges a goal and EMITS A CERTIFICATE the tiny trusted kernel (check.beta) validates. Given an
# intuitionistic propositional goal over the {-> , &} fragment, this searches for a natural-deduction
# proof and prints the check.beta certificate `<goal> <proof>`; the kernel checks it. The prover is
# UNTRUSTED: it is SOUND by construction (every rule it applies is a valid typing rule, so check.beta
# accepts every proof it emits) but only as complete as its fuel -- exactly the "automation on the
# untrusted side, authority in the kernel" split. Prints "unprovable" if the bounded search fails.
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
        if head in ("->", "&"):
            tokens.pop(0)
            a = parse(tokens)
            b = parse(tokens)
            assert tokens.pop(0) == ")"
            return (head, a, b)
        raise ValueError("bad prop head: %s" % head)
    return ("at", t)  # an atom name (a bare uppercase ident)


def beta_prop(p):
    if p[0] == "at":
        return p[1]
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


def prove(ctx, goal, fuel):
    if fuel <= 0:
        return None
    sat = saturate(ctx)
    # axiom: the goal is already in (the saturation of) the context
    direct = has(sat, goal)
    if direct is not None:
        return direct
    # R&: prove each conjunct
    if goal[0] == "&":
        la = prove(ctx, goal[1], fuel - 1)
        lb = prove(ctx, goal[2], fuel - 1)
        if la is not None and lb is not None:
            return ("pair", la, lb)
    # R->: assume the antecedent, prove the consequent
    if goal[0] == "->":
        nm = fresh()
        body = prove(ctx + [(goal[1], ("hyp", nm))], goal[2], fuel - 1)
        if body is not None:
            return ("lam", nm, goal[1], body)
    # L->: backward chaining through an implication hypothesis (modus ponens), fuel-bounded
    for prop, term in sat:
        if prop[0] == "->":
            arg = prove(ctx, prop[1], fuel - 1)
            if arg is not None:
                got = (prop[2], ("app", term, arg))
                if got[0] != "->" or got[1] != prop[1]:  # avoid trivially re-deriving the same implication
                    body = prove(ctx + [got], goal, fuel - 1)
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
    return "(%s %s %s)" % (h, to_db(term[1], binders), to_db(term[2], binders))


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
    if d <= 0 or rng.random() < 0.34:
        return ("at", rng.choice(atoms))
    return (rng.choice(("->", "&")), random_prop(d - 1, rng), random_prop(d - 1, rng))


def batch(n, seed, fuel):
    # generate n random {->,&} goals; for every one the prover discharges, print "<goal>\t<cert>" on one
    # line (a single process -> the test pipes each cert to check.beta). The rest are silently dropped.
    import random
    rng = random.Random(seed)
    for _ in range(n):
        goal = random_prop(rng.randint(1, 4), rng)
        proof = prove([], goal, fuel)
        if proof is not None:
            print("%s\t%s %s" % (beta_prop(goal), beta_prop(goal), to_db(proof, [])))


def main():
    if sys.argv[1] == "--gen":
        gen(int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 1)
        return
    if sys.argv[1] == "--batch":
        batch(int(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 1, 40)
        return
    goal = parse(tokenize(sys.argv[1]))
    proof = prove([], goal, int(sys.argv[2]) if len(sys.argv) > 2 else 40)
    if proof is None:
        print("unprovable")
    else:
        print("%s %s" % (beta_prop(goal), to_db(proof, [])))


if __name__ == "__main__":
    main()
