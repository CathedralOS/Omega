#!/usr/bin/env python3
# Random broad coverage of the checker DIAMOND on PROPOSITIONAL LOGIC proofs -- the ->/&/+/bot intro and
# elim rules (lam/hyp/app/pair/fst/snd/inl/inr/case/absurd). seam-fuzz and checker-diamond-fuzz cover the
# REDUCER and equality CONVERSION; this covers the LOGICAL typing rules, the one major checker subsystem
# otherwise cross-checked across all three checkers only at the ~25 hand-picked checker-diamond.sh cases
# (check.beta sees the logic heavily via the 200+ proof corpus, but checker.gamma / checker_typed.gamma do
# not). It instantiates a library of valid propositional-tautology schemas (id, K, projection, modus
# ponens, commutativity, currying, distributivity, ex falso, non-contradiction, double-negation-intro)
# with RANDOM distinct atoms, and requires all three trust-anchor checkers to ACCEPT each proof against
# its goal AND REJECT it against an atom-perturbed goal (the proof then has the wrong type). A single
# disagreement is a bug in one checker's logic. Deterministic (fixed seed).
import sys, random, subprocess

CHECK, INTERP, CGAMMA = sys.argv[1], sys.argv[2], sys.argv[3]
CTYPED = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4] else None  # type-erased checker_typed.gamma
K = int(sys.argv[5]) if len(sys.argv) > 5 else 120
CGDEFS = open(CGAMMA).read()
CTDEFS = open(CTYPED).read() if CTYPED else None
random.seed(20240607)

# Prop  : ("at",n) | ("->",a,b) | ("&",a,b) | ("+",a,b) | ("bot",)
# Proof : ("lam",prop,body) | ("hyp",i) | ("app",f,x) | ("pair",a,b) | ("fst",p) | ("snd",p)
#       | ("inl",prop,p) | ("inr",prop,p) | ("case",s,l,r) | ("absurd",prop,p)
# check.beta atoms are bare uppercase idents; checker.gamma atoms are (Atom n) -- same index either way.
ATOM = ["P", "Q", "R", "S", "T", "U", "V", "W"]
BOP_B = {"->": "->", "&": "&", "+": "+"}
BOP_G = {"->": "Arrow", "&": "And", "+": "Or"}


def beta_prop(p):
    if p[0] == "at":
        return ATOM[p[1]]
    if p[0] == "bot":
        return "(bot)"
    return "(%s %s %s)" % (BOP_B[p[0]], beta_prop(p[1]), beta_prop(p[2]))


def gamma_prop(p):
    if p[0] == "at":
        return "(Atom %d)" % p[1]
    if p[0] == "bot":
        return "Bot"
    return "(%s %s %s)" % (BOP_G[p[0]], gamma_prop(p[1]), gamma_prop(p[2]))


def beta_pf(x):
    h = x[0]
    if h == "hyp":
        return "(hyp %d)" % x[1]
    if h == "lam":
        return "(lam %s %s)" % (beta_prop(x[1]), beta_pf(x[2]))
    if h in ("fst", "snd"):
        return "(%s %s)" % (h, beta_pf(x[1]))
    if h in ("app", "pair"):
        return "(%s %s %s)" % (h, beta_pf(x[1]), beta_pf(x[2]))
    if h in ("inl", "inr", "absurd"):
        return "(%s %s %s)" % (h, beta_prop(x[1]), beta_pf(x[2]))
    return "(case %s %s %s)" % (beta_pf(x[1]), beta_pf(x[2]), beta_pf(x[3]))


def gamma_pf(x):
    h = x[0]
    G = {"hyp": "Hyp", "lam": "Lam", "app": "App", "pair": "Pair", "fst": "Fst",
         "snd": "Snd", "inl": "Inl", "inr": "Inr", "case": "Case", "absurd": "Absurd"}[h]
    if h == "hyp":
        return "(Hyp %d)" % x[1]
    if h == "lam":
        return "(Lam %s %s)" % (gamma_prop(x[1]), gamma_pf(x[2]))
    if h in ("fst", "snd"):
        return "(%s %s)" % (G, gamma_pf(x[1]))
    if h in ("app", "pair"):
        return "(%s %s %s)" % (G, gamma_pf(x[1]), gamma_pf(x[2]))
    if h in ("inl", "inr", "absurd"):
        return "(%s %s %s)" % (G, gamma_prop(x[1]), gamma_pf(x[2]))
    return "(Case %s %s %s)" % (gamma_pf(x[1]), gamma_pf(x[2]), gamma_pf(x[3]))


def schemas(a, b, c):  # each: (name, goal, proof) -- valid intuitionistic tautologies (see checker-diamond.sh)
    A, B, C, BOT = ("at", a), ("at", b), ("at", c), ("bot",)
    return [
        ("id", ("->", A, A), ("lam", A, ("hyp", 0))),
        ("K", ("->", A, ("->", B, A)), ("lam", A, ("lam", B, ("hyp", 1)))),
        ("fst", ("->", ("&", A, B), A), ("lam", ("&", A, B), ("fst", ("hyp", 0)))),
        ("snd", ("->", ("&", A, B), B), ("lam", ("&", A, B), ("snd", ("hyp", 0)))),
        ("and-comm", ("->", ("&", A, B), ("&", B, A)),
         ("lam", ("&", A, B), ("pair", ("snd", ("hyp", 0)), ("fst", ("hyp", 0))))),
        ("mp", ("->", ("&", ("->", A, B), A), B),
         ("lam", ("&", ("->", A, B), A), ("app", ("fst", ("hyp", 0)), ("snd", ("hyp", 0))))),
        ("inl", ("->", A, ("+", A, B)), ("lam", A, ("inl", B, ("hyp", 0)))),
        ("inr", ("->", B, ("+", A, B)), ("lam", B, ("inr", A, ("hyp", 0)))),
        ("or-comm", ("->", ("+", A, B), ("+", B, A)),
         ("lam", ("+", A, B), ("case", ("hyp", 0),
                               ("lam", A, ("inr", B, ("hyp", 0))),
                               ("lam", B, ("inl", A, ("hyp", 0)))))),
        ("curry", ("->", ("->", ("&", A, B), C), ("->", A, ("->", B, C))),
         ("lam", ("->", ("&", A, B), C),
          ("lam", A, ("lam", B, ("app", ("hyp", 2), ("pair", ("hyp", 1), ("hyp", 0))))))),
        ("dist", ("->", ("&", A, ("+", B, C)), ("+", ("&", A, B), ("&", A, C))),
         ("lam", ("&", A, ("+", B, C)),
          ("case", ("snd", ("hyp", 0)),
           ("lam", B, ("inl", ("&", A, C), ("pair", ("fst", ("hyp", 1)), ("hyp", 0)))),
           ("lam", C, ("inr", ("&", A, B), ("pair", ("fst", ("hyp", 1)), ("hyp", 0))))))),
        ("exfalso", ("->", BOT, A), ("lam", BOT, ("absurd", A, ("hyp", 0)))),
        ("noncontra", ("->", ("&", A, ("->", A, BOT)), BOT),
         ("lam", ("&", A, ("->", A, BOT)), ("app", ("snd", ("hyp", 0)), ("fst", ("hyp", 0))))),
        ("dni", ("->", A, ("->", ("->", A, BOT), BOT)),
         ("lam", A, ("lam", ("->", A, BOT), ("app", ("hyp", 0), ("hyp", 1))))),
    ]


def mutate(goal, fresh):  # replace one atom occurrence with a fresh index -> a structurally different prop
    n = [0]
    def count(p):
        if p[0] == "at":
            n[0] += 1
        elif p[0] in ("->", "&", "+"):
            count(p[1]); count(p[2])
    count(goal)
    if n[0] == 0:
        return None  # no atom to perturb (all-bot) -- skip the negative case
    j = random.randrange(n[0])
    ctr = [0]
    def rb(p):
        if p[0] == "at":
            i = ctr[0]; ctr[0] += 1
            return ("at", fresh) if i == j else p
        if p[0] in ("->", "&", "+"):
            return (p[0], rb(p[1]), rb(p[2]))
        return p
    return rb(goal)


def beta_verdict(goal, proof):  # check.beta: 'accept' | 'reject'
    return subprocess.run([CHECK], input="%s %s" % (goal, proof), capture_output=True, text=True).stdout.strip()


def gamma_verdict(defs, check_expr):  # a gamma checker on the interpreter: exit 1 => accept
    r = subprocess.run([INTERP], input="%s\n%s\n" % (defs, check_expr), capture_output=True, text=True)
    return "accept" if r.returncode == 1 else "reject"


checks = 0
fails = 0
for i in range(K):
    a, b, c = random.sample(range(4), 3)  # distinct atoms; fresh = 4 (unused) for the perturbation
    name, goal, proof = random.choice(schemas(a, b, c))
    bpf, gpf = beta_pf(proof), gamma_pf(proof)
    for g, expect in ((goal, "accept"), (mutate(goal, 4), "reject")):
        if g is None:
            continue
        gp = gamma_prop(g)
        vb = beta_verdict(beta_prop(g), bpf)
        gexpr = "(check %s %s)" % (gpf, gp)
        vg = gamma_verdict(CGDEFS, gexpr)
        vt = gamma_verdict(CTDEFS, gexpr) if CTDEFS else expect
        checks += 1
        if not (vb == vg == vt == expect):
            fails += 1
            print("  FAIL %-10s %s : check.beta=%s checker.gamma=%s typed=%s expect=%s"
                  % (name, gp[:40], vb, vg, vt, expect))

oracles = "check.beta + checker.gamma" + (" + checker_typed.gamma" if CTDEFS else "")
print("logic-diamond fuzz (%d random propositional proofs, %d checks, oracles: %s): %d disagreements"
      % (K, checks, oracles, fails))
sys.exit(1 if fails else 0)
