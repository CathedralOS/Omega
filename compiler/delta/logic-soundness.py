#!/usr/bin/env python3
# LOGIC SOUNDNESS SEAM -- the propositional-logic pillar bridged to classical TRUTH.
#
# The soundness bridge has operational seams for three checker subsystems: semantics-diamond (equality),
# induction-soundness (inductive universals), predicate-soundness (the inductive predicates). The fourth
# subsystem -- propositional LOGIC (->/&/+/bot intro+elim, Curry-Howard) -- had only checker-vs-checker
# evidence (logic-diamond-fuzz), no bridge to an independent notion of TRUTH. This closes that gap.
#
# check.beta's logic is INTUITIONISTIC, and intuitionistic provability implies CLASSICAL validity (a
# strict subset). So for every proposition check.beta proves, an independent classical truth-table oracle
# must find it a TAUTOLOGY -- and a perturbed goal that the oracle finds a genuine NON-tautology must be
# REJECTED (the proof can't have the wrong type). Two independent routes -- a kernel typing derivation and
# a semantic decision -- agreeing is evidence the checker's logic is sound (not a proof; the meta-theorem
# is the open problem). If check.beta ever accepted a proof of a classically-invalid proposition, this
# catches it. Deterministic. Restricted to the propositional fragment (truth tables are classical, atomic).
import sys, random, subprocess, itertools

CHECK = sys.argv[1]
K = int(sys.argv[2]) if len(sys.argv) > 2 else 100
random.seed(20240611)

ATOM = ["P", "Q", "R", "S", "T", "U", "V", "W"]


def beta_prop(p):
    h = p[0]
    if h == "at":
        return ATOM[p[1]]
    if h == "bot":
        return "(bot)"
    return "(%s %s %s)" % ({"->": "->", "&": "&", "+": "+"}[h], beta_prop(p[1]), beta_prop(p[2]))


def beta_pf(x):  # proof terms (the propositional fragment of logic-diamond-fuzz's grammar)
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


def prop_schemas(a, b, c):  # valid intuitionistic (hence classical) propositional tautologies + proofs
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


def atoms_of(p, s):
    if p[0] == "at":
        s.add(p[1])
    elif p[0] in ("->", "&", "+"):
        atoms_of(p[1], s)
        atoms_of(p[2], s)


def ev(p, env):  # classical truth-value of a propositional formula under an atom assignment
    h = p[0]
    if h == "at":
        return env[p[1]]
    if h == "bot":
        return False
    if h == "->":
        return (not ev(p[1], env)) or ev(p[2], env)
    if h == "&":
        return ev(p[1], env) and ev(p[2], env)
    return ev(p[1], env) or ev(p[2], env)  # "+"


def taut(p):  # is the formula true under EVERY assignment of its atoms?
    s = set()
    atoms_of(p, s)
    s = sorted(s)
    return all(ev(p, dict(zip(s, bits))) for bits in itertools.product((False, True), repeat=len(s)))


def perturb(p, fresh):  # replace the first atom leaf with a fresh index (usually breaks the tautology)
    if p[0] == "at":
        return ("at", fresh), True
    if p[0] in ("->", "&", "+"):
        l, done = perturb(p[1], fresh)
        if done:
            return (p[0], l, p[2]), True
        r, done = perturb(p[2], fresh)
        return (p[0], p[1], r), done
    return p, False


def verdict(goal, proof):
    return subprocess.run([CHECK], input="%s %s" % (beta_prop(goal), beta_pf(proof)),
                          capture_output=True, text=True).stdout.strip()


checks = 0
fails = 0
for _ in range(K):
    a, b, c = random.sample(range(8), 3)
    for name, goal, proof in prop_schemas(a, b, c):
        # ACCEPT side: the kernel accepts the proof AND the goal is a classical tautology
        v = verdict(goal, proof)
        checks += 1
        if not (v == "accept" and taut(goal)):
            fails += 1
            print("  FAIL %s accept : check=%s tautology=%s" % (name, v, taut(goal)))
            continue
        # REJECT side: a perturbed goal the oracle finds a genuine NON-tautology must be rejected
        used = set()
        atoms_of(goal, used)
        fresh = next(i for i in range(8) if i not in used)
        bad, _ = perturb(goal, fresh)
        if not taut(bad):
            checks += 1
            if verdict(bad, proof) != "reject":
                fails += 1
                print("  FAIL %s reject : check accepted a classically-INVALID proposition!" % name)

print("logic soundness seam (propositional proofs: kernel derivation vs classical truth-table): "
      "%d checks, %d disagreements" % (checks, fails))
sys.exit(1 if fails else 0)
