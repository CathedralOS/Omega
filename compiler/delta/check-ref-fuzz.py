#!/usr/bin/env python3
# check-ref-fuzz.py CHECK_EXE K — differential-test the trust anchor's LOGIC (propositional AND first-order):
# for K random valid proofs (tautology-schema libraries instantiated with random atoms / predicate indices /
# witness terms), require check.beta and the independent reference check_ref.py to AGREE — both ACCEPT the
# proof against its true goal, and both REJECT it against an index-perturbed (wrong-type) goal. A disagreement
# is a logic bug in one of them. Deterministic (fixed seed). check_ref covers ->/&/+/bot (intro+elim) plus
# All/Exists with de Bruijn (gen/inst/wit/unpack); equality-conversion and induction are check.beta-only.
import sys, os, random, subprocess
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import check_ref

CHECK = sys.argv[1]
K = int(sys.argv[2]) if len(sys.argv) > 2 else 200
random.seed(20240702)
ATOM = ["P", "Q", "R", "S", "T", "U", "V", "W"]

def term(t):
    if t[0] == "z":  return "z"
    if t[0] == "s":  return "(s %s)" % term(t[1])
    if t[0] in ("p", "m"): return "(%s %s %s)" % (t[0], term(t[1]), term(t[2]))
    return "(v %d)" % t[1]

def pr(p):
    h = p[0]
    if h == "at":   return ATOM[p[1]]
    if h == "bot":  return "(bot)"
    if h == "eq":   return "(= %s %s)" % (term(p[1]), term(p[2]))
    if h == "pred": return "(Pred %d %s)" % (p[1], term(p[2]))
    if h == "rel":  return "(Rel %d %s %s)" % (p[1], term(p[2]), term(p[3]))
    if h in ("all", "ex"): return "(%s %s)" % ("All" if h == "all" else "Exists", pr(p[1]))
    return "(%s %s %s)" % (h, pr(p[1]), pr(p[2]))

def num(n):                                            # n -> s^n z
    t = ("z",)
    for _ in range(n):
        t = ("s", t)
    return t

def tval(t):                                           # value of a Peano p/m term
    if t[0] == "z":  return 0
    if t[0] == "s":  return 1 + tval(t[1])
    if t[0] == "p":  return tval(t[1]) + tval(t[2])
    return tval(t[1]) * tval(t[2])                      # m

def arith(rng, depth):                                 # random small Peano p/m expression (value <= ~30)
    if depth <= 0 or rng.random() < 0.5:
        return num(rng.randint(0, 4))
    op = "p" if rng.random() < 0.6 else "m"
    return (op, arith(rng, depth - 1), arith(rng, depth - 1))

def pf(x):
    h = x[0]
    if h == "hyp":                    return "(hyp %d)" % x[1]
    if h == "refl":                   return "(refl %s)" % term(x[1])
    if h == "lam":                    return "(lam %s %s)" % (pr(x[1]), pf(x[2]))
    if h in ("fst", "snd", "gen"):    return "(%s %s)" % (h, pf(x[1]))
    if h in ("app", "pair", "unpack"): return "(%s %s %s)" % (h, pf(x[1]), pf(x[2]))
    if h in ("inl", "inr", "absurd"): return "(%s %s %s)" % (h, pr(x[1]), pf(x[2]))
    if h == "inst":                   return "(inst %s %s)" % (pf(x[1]), term(x[2]))
    if h == "wit":                    return "(wit %s %s %s)" % (pr(x[1]), term(x[2]), pf(x[3]))
    return "(case %s %s %s)" % (pf(x[1]), pf(x[2]), pf(x[3]))

def prop_schemas(a, b, c):
    A, B, C, BOT = ("at", a), ("at", b), ("at", c), ("bot",)
    return [
        (("->", A, A), ("lam", A, ("hyp", 0))),
        (("->", A, ("->", B, A)), ("lam", A, ("lam", B, ("hyp", 1)))),
        (("->", ("&", A, B), A), ("lam", ("&", A, B), ("fst", ("hyp", 0)))),
        (("->", ("&", A, B), ("&", B, A)),
         ("lam", ("&", A, B), ("pair", ("snd", ("hyp", 0)), ("fst", ("hyp", 0))))),
        (("->", ("&", ("->", A, B), A), B),
         ("lam", ("&", ("->", A, B), A), ("app", ("fst", ("hyp", 0)), ("snd", ("hyp", 0))))),
        (("->", A, ("+", A, B)), ("lam", A, ("inl", B, ("hyp", 0)))),
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

def quant_schemas(pi, atomq, T):
    def P(t): return ("pred", pi, t)
    V0, Q, BOT = ("iv", 0), ("at", atomq), ("bot",)
    return [
        (("all", ("->", P(V0), P(V0))), ("gen", ("lam", P(V0), ("hyp", 0)))),
        (("->", ("all", P(V0)), P(T)), ("lam", ("all", P(V0)), ("inst", ("hyp", 0), T))),
        (("->", P(T), ("ex", P(V0))), ("lam", P(T), ("wit", P(V0), T, ("hyp", 0)))),
        (("->", ("ex", P(V0)), ("->", ("all", ("->", P(V0), Q)), Q)),
         ("lam", ("ex", P(V0)), ("lam", ("all", ("->", P(V0), Q)), ("unpack", ("hyp", 1), ("hyp", 0))))),
        (("->", ("->", ("ex", P(V0)), BOT), ("all", ("->", P(V0), BOT))),
         ("lam", ("->", ("ex", P(V0)), BOT),
          ("gen", ("lam", P(V0), ("app", ("hyp", 1), ("wit", P(V0), V0, ("hyp", 0))))))),
        (("->", ("all", ("->", P(V0), BOT)), ("->", ("ex", P(V0)), BOT)),
         ("lam", ("all", ("->", P(V0), BOT)),
          ("lam", ("ex", P(V0)),
           ("unpack", ("hyp", 0),
            ("gen", ("lam", P(V0), ("app", ("inst", ("hyp", 2), V0), ("hyp", 0)))))))),
        (("all", ("->", ("all", P(V0)), P(V0))),
         ("gen", ("lam", ("all", P(V0)), ("inst", ("hyp", 0), V0)))),
        (("all", ("->", P(V0), ("ex", P(V0)))),
         ("gen", ("lam", P(V0), ("wit", P(V0), V0, ("hyp", 0))))),
    ]

def mutate(goal, fresh):                               # bump one atom / pred index -> a wrong-type goal
    leaves = []
    def walk(p):
        if p[0] in ("at", "pred", "rel"): leaves.append(p)
        elif p[0] in ("->", "&", "+"): walk(p[1]); walk(p[2])
        elif p[0] in ("all", "ex"): walk(p[1])
    walk(goal)
    if not leaves:
        return None
    j = random.randrange(len(leaves)); ctr = [0]
    def rb(p):
        if p[0] in ("at", "pred", "rel"):
            i = ctr[0]; ctr[0] += 1
            return (p[0], fresh) + p[2:] if i == j else p
        if p[0] in ("->", "&", "+"):  return (p[0], rb(p[1]), rb(p[2]))
        if p[0] in ("all", "ex"):     return (p[0], rb(p[1]))
        return p
    return rb(goal)

def gterm(n):
    t = ("z",)
    for _ in range(n):
        t = ("s", t)
    return t

def verdict_beta(goal, proof):
    return subprocess.run([CHECK], input="%s %s" % (goal, proof), capture_output=True, text=True).stdout.strip()

def verdict_ref(goal, proof):
    g = check_ref.parse_all("%s %s" % (goal, proof))
    r = check_ref.infer(g[1], [], 0)                   # conversion-aware, matching check_ref's own main()
    return 'accept' if r is not None and check_ref.prop_eq(r, g[0]) else 'reject'

fails = 0; n = 0
for _ in range(K):
    r = random.random()
    if r < 0.4:                                        # propositional
        a, b, c = random.sample(range(6), 3)
        goal, proof = random.choice(prop_schemas(a, b, c))
        cases = [(goal, "accept"), (mutate(goal, 7), "reject")]
    elif r < 0.7:                                      # first-order (quantifiers)
        goal, proof = random.choice(quant_schemas(random.randrange(3), 5, gterm(random.randint(0, 2))))
        cases = [(goal, "accept"), (mutate(goal, 7), "reject")]
    else:                                              # equality / conversion (refl over Peano p/m)
        e = arith(random.Random(random.random()), 3); v = tval(e)
        proof = ("refl", num(v))
        cases = [(("eq", e, num(v)), "accept"), (("eq", e, num(v + 1)), "reject")]
    pfs = pf(proof)
    for g, expect in cases:
        if g is None:
            continue
        gs = pr(g); n += 1
        vb, vr = verdict_beta(gs, pfs), verdict_ref(gs, pfs)
        if not (vb == vr == expect):
            fails += 1
            print("  FAIL %s : beta=%s ref=%s want=%s : %s" % (gs, vb, vr, expect, pfs))
print("%d ok, %d failed" % (n - fails, fails))
sys.exit(1 if fails else 0)
