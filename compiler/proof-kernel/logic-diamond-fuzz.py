#!/usr/bin/env python3
# Random broad coverage of the checker DIAMOND on FIRST-ORDER LOGIC proofs -- both the propositional rules
# (->/&/+/bot intro+elim: lam/hyp/app/pair/fst/snd/inl/inr/case/absurd) AND the quantifier/predicate rules
# (All/Exists over Pred/Rel with de Bruijn vars: gen/inst/wit/unpack). seam-fuzz and checker-diamond-fuzz
# cover the REDUCER and equality CONVERSION; this covers the LOGICAL typing rules, the one major checker
# subsystem otherwise cross-checked across all three checkers only at the ~40 hand-picked checker-diamond.sh
# cases (check.beta sees the logic heavily via the 200+ proof corpus, but checker.gamma / checker_typed.gamma
# do not). It instantiates a library of valid tautology schemas with RANDOM atoms / predicate indices /
# witness terms, and requires all three trust-anchor checkers to ACCEPT each proof against its goal AND
# REJECT it against an index-perturbed goal (the proof then has the wrong type). A single disagreement is a
# bug in one checker's logic. Deterministic (fixed seed). The validated syntax lives in checker-diamond.sh.
import sys, random, subprocess

CHECK, INTERP, CGAMMA = sys.argv[1], sys.argv[2], sys.argv[3]
CTYPED = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4] else None  # type-erased checker_typed.gamma
K = int(sys.argv[5]) if len(sys.argv) > 5 else 120
CGDEFS = open(CGAMMA).read()
CTDEFS = open(CTYPED).read() if CTYPED else None
random.seed(20240607)

# Term  : ("z",) | ("s",t) | ("iv",i)                      -- iv = de Bruijn var of an enclosing All/Exists
# Prop  : ("at",n) | ("->",a,b) | ("&",a,b) | ("+",a,b) | ("bot",)
#       | ("pred",n,t) | ("rel",n,t,t) | ("all",body) | ("ex",body)
# Proof : ("lam",prop,p) | ("hyp",i) | ("app",f,x) | ("pair",a,b) | ("fst",p) | ("snd",p)
#       | ("inl",prop,p) | ("inr",prop,p) | ("case",s,l,r) | ("absurd",prop,p)
#       | ("gen",p) | ("inst",p,t) | ("wit",prop,t,p) | ("unpack",p,body)
# check.beta atoms are bare uppercase idents + (v i); checker.gamma uses (Atom n) + (Iv i).
ATOM = ["P", "Q", "R", "S", "T", "U", "V", "W"]
BOP_B = {"->": "->", "&": "&", "+": "+"}
BOP_G = {"->": "Arrow", "&": "And", "+": "Or"}


def beta_term(t):
    if t[0] == "z":
        return "z"
    if t[0] == "s":
        return "(s %s)" % beta_term(t[1])
    return "(v %d)" % t[1]


def gamma_term(t):
    if t[0] == "z":
        return "Ze"
    if t[0] == "s":
        return "(Su %s)" % gamma_term(t[1])
    return "(Iv %d)" % t[1]


def beta_prop(p):
    h = p[0]
    if h == "at":
        return ATOM[p[1]]
    if h == "bot":
        return "(bot)"
    if h == "pred":
        return "(Pred %d %s)" % (p[1], beta_term(p[2]))
    if h == "rel":
        return "(Rel %d %s %s)" % (p[1], beta_term(p[2]), beta_term(p[3]))
    if h in ("all", "ex"):
        return "(%s %s)" % ("All" if h == "all" else "Exists", beta_prop(p[1]))
    return "(%s %s %s)" % (BOP_B[h], beta_prop(p[1]), beta_prop(p[2]))


def gamma_prop(p):
    h = p[0]
    if h == "at":
        return "(Atom %d)" % p[1]
    if h == "bot":
        return "Bot"
    if h == "pred":
        return "(Pred %d %s)" % (p[1], gamma_term(p[2]))
    if h == "rel":
        return "(Rel %d %s %s)" % (p[1], gamma_term(p[2]), gamma_term(p[3]))
    if h in ("all", "ex"):
        return "(%s %s)" % ("All" if h == "all" else "Exists", gamma_prop(p[1]))
    return "(%s %s %s)" % (BOP_G[h], gamma_prop(p[1]), gamma_prop(p[2]))


def beta_pf(x):
    h = x[0]
    if h == "hyp":
        return "(hyp %d)" % x[1]
    if h == "lam":
        return "(lam %s %s)" % (beta_prop(x[1]), beta_pf(x[2]))
    if h in ("fst", "snd", "gen"):
        return "(%s %s)" % (h, beta_pf(x[1]))
    if h in ("app", "pair", "unpack"):
        return "(%s %s %s)" % (h, beta_pf(x[1]), beta_pf(x[2]))
    if h in ("inl", "inr", "absurd"):
        return "(%s %s %s)" % (h, beta_prop(x[1]), beta_pf(x[2]))
    if h == "inst":
        return "(inst %s %s)" % (beta_pf(x[1]), beta_term(x[2]))
    if h == "wit":
        return "(wit %s %s %s)" % (beta_prop(x[1]), beta_term(x[2]), beta_pf(x[3]))
    return "(case %s %s %s)" % (beta_pf(x[1]), beta_pf(x[2]), beta_pf(x[3]))


def gamma_pf(x):
    h = x[0]
    G = {"lam": "Lam", "app": "App", "pair": "Pair", "fst": "Fst", "snd": "Snd", "inl": "Inl",
         "inr": "Inr", "absurd": "Absurd", "gen": "Gen", "unpack": "Unpack"}
    if h == "hyp":
        return "(Hyp %d)" % x[1]
    if h == "lam":
        return "(Lam %s %s)" % (gamma_prop(x[1]), gamma_pf(x[2]))
    if h in ("fst", "snd", "gen"):
        return "(%s %s)" % (G[h], gamma_pf(x[1]))
    if h in ("app", "pair", "unpack"):
        return "(%s %s %s)" % (G[h], gamma_pf(x[1]), gamma_pf(x[2]))
    if h in ("inl", "inr", "absurd"):
        return "(%s %s %s)" % (G[h], gamma_prop(x[1]), gamma_pf(x[2]))
    if h == "inst":
        return "(Inst %s %s)" % (gamma_pf(x[1]), gamma_term(x[2]))
    if h == "wit":
        return "(Wit %s %s %s)" % (gamma_prop(x[1]), gamma_term(x[2]), gamma_pf(x[3]))
    return "(Case %s %s %s)" % (gamma_pf(x[1]), gamma_pf(x[2]), gamma_pf(x[3]))


def gterm(n):  # ground witness term: a Peano numeral
    t = ("z",)
    for _ in range(n):
        t = ("s", t)
    return t


def prop_schemas(a, b, c):  # propositional tautologies (see checker-diamond.sh lines 67-122)
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


def quant_schemas(p, q, atomq, T):  # quantifier/predicate tautologies (see checker-diamond.sh lines 85-119)
    def P(t):
        return ("pred", p, t)
    V0, Q, BOT = ("iv", 0), ("at", atomq), ("bot",)
    return [
        ("forall-id", ("all", ("->", P(V0), P(V0))), ("gen", ("lam", P(V0), ("hyp", 0)))),
        ("forall-elim", ("->", ("all", P(V0)), P(T)),
         ("lam", ("all", P(V0)), ("inst", ("hyp", 0), T))),
        ("exists-intro", ("->", P(T), ("ex", P(V0))),
         ("lam", P(T), ("wit", P(V0), T, ("hyp", 0)))),
        ("exists-elim", ("->", ("ex", P(V0)), ("->", ("all", ("->", P(V0), Q)), Q)),
         ("lam", ("ex", P(V0)), ("lam", ("all", ("->", P(V0), Q)),
                                 ("unpack", ("hyp", 1), ("hyp", 0))))),
        ("neg-ex-all-neg",
         ("->", ("->", ("ex", P(V0)), BOT), ("all", ("->", P(V0), BOT))),
         ("lam", ("->", ("ex", P(V0)), BOT),
          ("gen", ("lam", P(V0), ("app", ("hyp", 1), ("wit", P(V0), V0, ("hyp", 0))))))),
        ("all-neg-neg-ex",
         ("->", ("all", ("->", P(V0), BOT)), ("->", ("ex", P(V0)), BOT)),
         ("lam", ("all", ("->", P(V0), BOT)),
          ("lam", ("ex", P(V0)),
           ("unpack", ("hyp", 0),
            ("gen", ("lam", P(V0), ("app", ("inst", ("hyp", 2), V0), ("hyp", 0)))))))),
        # NESTED quantifier props (a quantifier inside another quantifier's body) -- exercises de Bruijn
        # across binder depth and inst/wit under an outer All.
        ("forall-nest-elim", ("all", ("->", ("all", P(V0)), P(V0))),
         ("gen", ("lam", ("all", P(V0)), ("inst", ("hyp", 0), V0)))),
        ("forall-exists-intro", ("all", ("->", P(V0), ("ex", P(V0)))),
         ("gen", ("lam", P(V0), ("wit", P(V0), V0, ("hyp", 0))))),
    ]


def mutate(goal, fresh):  # bump one atom/pred/rel index -> a structurally different (wrong-type) goal
    def leaves(p):
        if p[0] in ("at", "pred", "rel"):
            return [p]
        if p[0] in ("->", "&", "+"):
            return leaves(p[1]) + leaves(p[2])
        if p[0] in ("all", "ex"):
            return leaves(p[1])
        return []
    n = len(leaves(goal))
    if n == 0:
        return None  # nothing to perturb (e.g. an all-bot goal) -- skip the negative case
    j = random.randrange(n)
    ctr = [0]
    def rb(p):
        if p[0] in ("at", "pred", "rel"):
            i = ctr[0]; ctr[0] += 1
            return (p[0], fresh) + p[2:] if i == j else p   # keep term args, bump only the index
        if p[0] in ("->", "&", "+"):
            return (p[0], rb(p[1]), rb(p[2]))
        if p[0] in ("all", "ex"):
            return (p[0], rb(p[1]))
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
    if random.random() < 0.5:  # propositional
        a, b, c = random.sample(range(4), 3)
        name, goal, proof = random.choice(prop_schemas(a, b, c))
    else:                      # first-order (quantifiers + predicates)
        p = random.randrange(3)
        name, goal, proof = random.choice(quant_schemas(p, None, 5, gterm(random.randint(0, 2))))
    bpf, gpf = beta_pf(proof), gamma_pf(proof)
    for g, expect in ((goal, "accept"), (mutate(goal, 7), "reject")):
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
            print("  FAIL %-14s %s : check.beta=%s checker.gamma=%s typed=%s expect=%s"
                  % (name, gp[:44], vb, vg, vt, expect))

oracles = "check.beta + checker.gamma" + (" + checker_typed.gamma" if CTDEFS else "")
print("logic-diamond fuzz (%d random first-order proofs, %d checks, oracles: %s): %d disagreements"
      % (K, checks, oracles, fails))
sys.exit(1 if fails else 0)
