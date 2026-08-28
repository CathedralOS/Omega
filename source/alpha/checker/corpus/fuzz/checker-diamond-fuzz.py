#!/usr/bin/env python3
# Random broad coverage of the CHECKER DIAMOND: the two independently-written trust-anchor checkers --
# check.beta (Beta; integer-tag node trees) and checker.gamma (Gamma; algebraic data + pattern matching,
# run on the reference interpreter) -- must agree on EVERY certificate. checker-diamond.sh cross-checks
# them at ~83 curated cases; this runs hundreds of random ones. For each generated closed Peano/List
# equation E = V it builds the equality proposition with a `refl` proof and requires BOTH checkers to
# ACCEPT the true one (E = value(E)) and REJECT a perturbed one (E = value(E)+1, or a prepended-element
# list). A single disagreement is a bug (or a backdoor) in one checker. Deterministic (fixed seed).
import sys, random, subprocess

CHECK, INTERP, CGAMMA = sys.argv[1], sys.argv[2], sys.argv[3]
CTYPED = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4] else None  # type-erased checker_typed.gamma (3rd oracle)
K = int(sys.argv[5]) if len(sys.argv) > 5 else 120
CGDEFS = open(CGAMMA).read()
CTDEFS = open(CTYPED).read() if CTYPED else None
random.seed(20240607)

# check.beta term syntax vs checker.gamma term syntax (NB: gamma uses Pl/Mu/Lapp/Llen, not plus/mult).
BETA = {"z": "z", "s": "s", "+": "p", "*": "m", "len": "len", "nil": "nil", "cons": "cons", "app": "app"}
GAMMA = {"z": "Ze", "s": "Su", "+": "Pl", "*": "Mu", "len": "Llen", "nil": "Lnil", "cons": "Lcons", "app": "Lapp"}


def gen_nat(d):
    if d <= 0 or random.random() < 0.4:
        return ("lit", random.randint(0, 4))
    r = random.random()
    if r < 0.3:
        return ("s", gen_nat(d - 1))
    if r < 0.55:
        return ("+", gen_nat(d - 1), gen_nat(d - 1))
    if r < 0.8:
        return ("*", gen_nat(d - 1), gen_nat(d - 1))
    return ("len", gen_list(d - 1))


def gen_list(d):
    if d <= 0 or random.random() < 0.45:
        out = ("nil",)
        for _ in range(random.randint(0, 3)):
            out = ("cons", ("lit", random.randint(0, 3)), out)
        return out
    if random.random() < 0.5:
        return ("cons", gen_nat(d - 1), gen_list(d - 1))
    return ("app", gen_list(d - 1), gen_list(d - 1))


def val_nat(e):
    if e[0] == "lit":
        return e[1]
    if e[0] == "s":
        return val_nat(e[1]) + 1
    if e[0] == "+":
        return val_nat(e[1]) + val_nat(e[2])
    if e[0] == "*":
        return val_nat(e[1]) * val_nat(e[2])
    return len(val_list(e[1]))


def val_list(e):
    if e[0] == "nil":
        return []
    if e[0] == "cons":
        return [val_nat(e[1])] + val_list(e[2])
    return val_list(e[1]) + val_list(e[2])


def render(e, T):
    t = e[0]
    if t == "lit":
        return rnat(e[1], T)
    if t == "s":
        return "(%s %s)" % (T["s"], render(e[1], T))
    if t in ("+", "*"):
        return "(%s %s %s)" % (T[t], render(e[1], T), render(e[2], T))
    if t == "len":
        return "(%s %s)" % (T["len"], render(e[1], T))
    if t == "nil":
        return T["nil"]
    if t == "cons":
        return "(%s %s %s)" % (T["cons"], render(e[1], T), render(e[2], T))
    return "(%s %s %s)" % (T["app"], render(e[1], T), render(e[2], T))


def rnat(n, T):
    out = T["z"]
    for _ in range(n):
        out = "(%s %s)" % (T["s"], out)
    return out


def rlist(lst, T):
    out = T["nil"]
    for x in reversed(lst):
        out = "(%s %s %s)" % (T["cons"], rnat(x, T), out)
    return out


def beta_verdict(goal, proof):  # check.beta: 'accept' | 'reject'
    return subprocess.run([CHECK], input="%s %s" % (goal, proof), capture_output=True, text=True).stdout.strip()


def gamma_verdict(defs, check_expr):  # a gamma checker on the interpreter: exit 1 => accept
    r = subprocess.run([INTERP], input="%s\n%s\n" % (defs, check_expr), capture_output=True, text=True)
    return "accept" if r.returncode == 1 else "reject"


def make_case():
    if random.random() < 0.45:
        e = gen_list(random.randint(1, 4))
        v = val_list(e)
        if len(v) > 8 or any(x > 8 for x in v):
            return None
        eb, eg = render(e, BETA), render(e, GAMMA)
        return (eb, eg, rlist(v, BETA), rlist(v, GAMMA), rlist([0] + v, BETA), rlist([0] + v, GAMMA))
    e = gen_nat(random.randint(1, 4))
    v = val_nat(e)
    if v > 36:
        return None
    eb, eg = render(e, BETA), render(e, GAMMA)
    return (eb, eg, rnat(v, BETA), rnat(v, GAMMA), rnat(v + 1, BETA), rnat(v + 1, GAMMA))


checks = 0
fails = 0
i = 0
attempts = 0
while i < K and attempts < K * 8:
    attempts += 1
    c = make_case()
    if c is None:
        continue
    i += 1
    eb, eg, tb, tg, fb, fg = c
    # true: E = value(E)  -> both must ACCEPT ; false: E = value(E)+/-perturb -> both must REJECT
    for rb, rg, expect in ((tb, tg, "accept"), (fb, fg, "reject")):
        gexpr = "(check (Refl %s) (Eq %s %s))" % (eg, eg, rg)
        vb = beta_verdict("(= %s %s)" % (eb, rb), "(refl %s)" % eb)
        vg = gamma_verdict(CGDEFS, gexpr)
        vt = gamma_verdict(CTDEFS, gexpr) if CTDEFS else expect  # type-erased checker_typed.gamma
        checks += 1
        if not (vb == vg == vt == expect):
            fails += 1
            print("  FAIL  (= %s %s) : check.beta=%s checker.gamma=%s typed=%s expect=%s"
                  % (eb[:30], rb[:16], vb, vg, vt, expect))

oracles = "check.beta + checker.gamma" + (" + checker_typed.gamma" if CTDEFS else "")
print("checker-diamond fuzz (%d random Peano/List equations, %d checks, oracles: %s): %d disagreements"
      % (i, checks, oracles, fails))
sys.exit(1 if fails else 0)
