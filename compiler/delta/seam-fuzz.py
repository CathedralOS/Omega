#!/usr/bin/env python3
# Random broad coverage of the gamma/delta soundness seam. Generates closed expressions over BOTH
# domains the checker reduces -- Peano naturals {z, s, +, *, length} and Lists {nil, cons, append} --
# and requires the checker's DEFINITIONAL equality (eq.beta: normalize both sides) and the reference
# interpreter's OPERATIONAL evaluation (interp.beta running gamma's own plus/mult/append/length) to
# AGREE on each: both that `E = value(E)` (verdict "equal") and that `E != value(E)`-perturbed (verdict
# "differ"). Nat results are compared with neq, List results with leq. A single disagreement is a
# soundness break at the seam. Deterministic (fixed seed), so it is reproducible.
import sys, random, subprocess

EQ, INTERP = sys.argv[1], sys.argv[2]
K = int(sys.argv[3]) if len(sys.argv) > 3 else 120
DEFS = (
    '(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) '
    '(def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) '
    '(def neq (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (neq x y)) (w 0))))) '
    '(def append (a b) (match a (Lnil b) ((Lcons h t) (Lcons h (append t b))))) '
    '(def length (l) (match l (Lnil Ze) ((Lcons h t) (Su (length t))))) '
    '(def leq (a b) (match a (Lnil (match b (Lnil 1) (w 0))) ((Lcons h t) (match b ((Lcons i u) (if (neq h i) (leq t u) 0)) (w 0)))))'
)
random.seed(20240607)  # deterministic -- reproducible cases every run

DELTA = {"z": "z", "s": "s", "+": "p", "*": "m", "len": "len", "nil": "nil", "cons": "cons", "app": "app"}
GAMMA = {"z": "Ze", "s": "Su", "+": "plus", "*": "mult", "len": "length", "nil": "Lnil", "cons": "Lcons", "app": "append"}


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
    return len(val_list(e[1]))  # len


def val_list(e):
    if e[0] == "nil":
        return []
    if e[0] == "cons":
        return [val_nat(e[1])] + val_list(e[2])
    return val_list(e[1]) + val_list(e[2])  # app


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
    return "(%s %s %s)" % (T["app"], render(e[1], T), render(e[2], T))  # app


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


def eq_verdict(d1, d2):
    return subprocess.run([EQ], input="%s %s" % (d1, d2), capture_output=True, text=True).stdout.strip()


def op_verdict(rel, g1, g2):  # rel = "neq" (Nat) or "leq" (List); exit 1 => equal
    r = subprocess.run([INTERP], input="%s\n(%s %s %s)\n" % (DEFS, rel, g1, g2), capture_output=True, text=True)
    return "equal" if r.returncode == 1 else "differ"


checks = 0
fails = 0
i = 0
attempts = 0
while i < K and attempts < K * 8:
    attempts += 1
    is_list = random.random() < 0.45
    e = gen_list(random.randint(1, 4)) if is_list else gen_nat(random.randint(1, 4))
    if is_list:
        v = val_list(e)
        if len(v) > 8 or any(x > 8 for x in v):
            continue
        same, diff, rel = rlist(v, DELTA), rlist([0] + v, DELTA), "leq"
        same_g, diff_g = rlist(v, GAMMA), rlist([0] + v, GAMMA)
    else:
        v = val_nat(e)
        if v > 36:
            continue
        same, diff, rel = rnat(v, DELTA), rnat(v + 1, DELTA), "neq"
        same_g, diff_g = rnat(v, GAMMA), rnat(v + 1, GAMMA)
    i += 1
    ed, eg = render(e, DELTA), render(e, GAMMA)
    for rhs_d, rhs_g, expect in ((same, same_g, "equal"), (diff, diff_g, "differ")):
        veq = eq_verdict(ed, rhs_d)
        vop = op_verdict(rel, eg, rhs_g)
        checks += 1
        if not (veq == vop == expect):
            fails += 1
            print("  FAIL  %s vs %s : definitional=%s operational=%s expect=%s" % (ed[:40], rhs_d[:24], veq, vop, expect))

print("seam fuzz (%d random naturals AND lists over +/*/append/length, %d checks): %d disagreements" % (i, checks, fails))
sys.exit(1 if fails else 0)
