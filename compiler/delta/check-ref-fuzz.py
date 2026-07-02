#!/usr/bin/env python3
# check-ref-fuzz.py CHECK_EXE K — differential-test the trust anchor's PROPOSITIONAL logic: for K random
# valid propositional proofs (a tautology-schema library instantiated with random atoms), require check.beta
# and the independent reference check_ref.py to AGREE — both ACCEPT the proof against its true goal, and both
# REJECT it against an index-perturbed (wrong-type) goal. A disagreement is a propositional-logic bug in one
# of them. Deterministic (fixed seed). Mirrors logic-diamond-fuzz.py's approach with a fourth, independent
# (Python) checker for the propositional fragment.
import sys, os, random, subprocess
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import check_ref

CHECK = sys.argv[1]
K = int(sys.argv[2]) if len(sys.argv) > 2 else 200
random.seed(20240702)
ATOM = ["P", "Q", "R", "S", "T", "U", "V", "W"]

def pr(p):                                             # render a prop tuple -> check.beta string
    h = p[0]
    if h == "at":   return ATOM[p[1]]
    if h == "bot":  return "(bot)"
    return "(%s %s %s)" % (h, pr(p[1]), pr(p[2]))

def pf(x):                                             # render a proof tuple -> check.beta string
    h = x[0]
    if h == "hyp":                 return "(hyp %d)" % x[1]
    if h == "lam":                 return "(lam %s %s)" % (pr(x[1]), pf(x[2]))
    if h in ("fst", "snd"):        return "(%s %s)" % (h, pf(x[1]))
    if h in ("app", "pair"):       return "(%s %s %s)" % (h, pf(x[1]), pf(x[2]))
    if h in ("inl", "inr", "absurd"): return "(%s %s %s)" % (h, pr(x[1]), pf(x[2]))
    return "(case %s %s %s)" % (pf(x[1]), pf(x[2]), pf(x[3]))

def schemas(a, b, c):
    A, B, C, BOT = ("at", a), ("at", b), ("at", c), ("bot",)
    return [
        (("->", A, A), ("lam", A, ("hyp", 0))),
        (("->", A, ("->", B, A)), ("lam", A, ("lam", B, ("hyp", 1)))),
        (("->", ("&", A, B), A), ("lam", ("&", A, B), ("fst", ("hyp", 0)))),
        (("->", ("&", A, B), B), ("lam", ("&", A, B), ("snd", ("hyp", 0)))),
        (("->", ("&", A, B), ("&", B, A)),
         ("lam", ("&", A, B), ("pair", ("snd", ("hyp", 0)), ("fst", ("hyp", 0))))),
        (("->", ("&", ("->", A, B), A), B),
         ("lam", ("&", ("->", A, B), A), ("app", ("fst", ("hyp", 0)), ("snd", ("hyp", 0))))),
        (("->", A, ("+", A, B)), ("lam", A, ("inl", B, ("hyp", 0)))),
        (("->", B, ("+", A, B)), ("lam", B, ("inr", A, ("hyp", 0)))),
        (("->", ("+", A, B), ("+", B, A)),
         ("lam", ("+", A, B), ("case", ("hyp", 0), ("lam", A, ("inr", B, ("hyp", 0))),
                                                    ("lam", B, ("inl", A, ("hyp", 0)))))),
        (("->", ("->", ("&", A, B), C), ("->", A, ("->", B, C))),
         ("lam", ("->", ("&", A, B), C),
          ("lam", A, ("lam", B, ("app", ("hyp", 2), ("pair", ("hyp", 1), ("hyp", 0))))))),
        (("->", ("&", A, ("+", B, C)), ("+", ("&", A, B), ("&", A, C))),
         ("lam", ("&", A, ("+", B, C)),
          ("case", ("snd", ("hyp", 0)),
           ("lam", B, ("inl", ("&", A, C), ("pair", ("fst", ("hyp", 1)), ("hyp", 0)))),
           ("lam", C, ("inr", ("&", A, B), ("pair", ("fst", ("hyp", 1)), ("hyp", 0))))))),
        (("->", BOT, A), ("lam", BOT, ("absurd", A, ("hyp", 0)))),
        (("->", ("&", A, ("->", A, BOT)), BOT),
         ("lam", ("&", A, ("->", A, BOT)), ("app", ("snd", ("hyp", 0)), ("fst", ("hyp", 0))))),
        (("->", A, ("->", ("->", A, BOT), BOT)),
         ("lam", A, ("lam", ("->", A, BOT), ("app", ("hyp", 0), ("hyp", 1))))),
    ]

def mutate(goal, fresh):                               # bump one atom index -> a wrong-type goal
    leaves = []
    def walk(p):
        if p[0] == "at": leaves.append(p)
        elif p[0] in ("->", "&", "+"): walk(p[1]); walk(p[2])
    walk(goal)
    if not leaves:
        return None
    j = random.randrange(len(leaves)); ctr = [0]
    def rb(p):
        if p[0] == "at":
            i = ctr[0]; ctr[0] += 1
            return ("at", fresh) if i == j else p
        if p[0] in ("->", "&", "+"):
            return (p[0], rb(p[1]), rb(p[2]))
        return p
    return rb(goal)

def verdict_beta(goal, proof):
    return subprocess.run([CHECK], input="%s %s" % (goal, proof), capture_output=True, text=True).stdout.strip()

def verdict_ref(goal, proof):
    g = check_ref.parse_all("%s %s" % (goal, proof))
    return 'accept' if check_ref.infer(g[1], []) == g[0] else 'reject'

fails = 0; n = 0
for _ in range(K):
    a, b, c = random.sample(range(6), 3)
    goal, proof = random.choice(schemas(a, b, c))
    pfs = pf(proof)
    for g, expect in ((goal, "accept"), (mutate(goal, 7), "reject")):
        if g is None:
            continue
        gs = pr(g); n += 1
        vb, vr = verdict_beta(gs, pfs), verdict_ref(gs, pfs)
        if not (vb == vr == expect):
            fails += 1
            print("  FAIL %s : beta=%s ref=%s want=%s : %s" % (gs, vb, vr, expect, pfs))
print("%d ok, %d failed" % (n - fails, fails))
sys.exit(1 if fails else 0)
