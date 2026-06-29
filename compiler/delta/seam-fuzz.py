#!/usr/bin/env python3
# Random broad coverage of the gamma/delta soundness seam. Generates closed Peano expressions over
# {z, s, +, *} and requires the checker's DEFINITIONAL equality (eq.beta, normalize both sides) and
# the interpreter's OPERATIONAL evaluation (interp.beta running gamma's own plus/mult) to AGREE on
# each: both that E = value(E) (verdict "equal") and that E != value(E)+1 (verdict "differ"). A single
# disagreement is a soundness break at the seam. Deterministic (fixed seed), so it is reproducible.
import sys, random, subprocess

EQ, INTERP = sys.argv[1], sys.argv[2]
K = int(sys.argv[3]) if len(sys.argv) > 3 else 120
DEFS = ('(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) '
        '(def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) '
        '(def neq (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (neq x y)) (w 0)))))')
random.seed(20240607)  # deterministic — reproducible cases every run


def gen(depth):
    if depth <= 0 or random.random() < 0.35:
        return ("lit", random.randint(0, 4))
    r = random.random()
    if r < 0.34:
        return ("s", gen(depth - 1))
    if r < 0.67:
        return ("+", gen(depth - 1), gen(depth - 1))
    return ("*", gen(depth - 1), gen(depth - 1))


def val(e):
    if e[0] == "lit":
        return e[1]
    if e[0] == "s":
        return val(e[1]) + 1
    if e[0] == "+":
        return val(e[1]) + val(e[2])
    return val(e[1]) * val(e[2])


def unary(n, z, s):
    out = z
    for _ in range(n):
        out = "(%s %s)" % (s, out)
    return out


def render(e, z, s, add, mul):
    if e[0] == "lit":
        return unary(e[1], z, s)
    if e[0] == "s":
        return "(%s %s)" % (s, render(e[1], z, s, add, mul))
    op = add if e[0] == "+" else mul
    return "(%s %s %s)" % (op, render(e[1], z, s, add, mul), render(e[2], z, s, add, mul))


def eq_verdict(d1, d2):  # eq.beta: 'equal' | 'differ'
    return subprocess.run([EQ], input="%s %s" % (d1, d2), capture_output=True, text=True).stdout.strip()


def op_verdict(g1, g2):  # interp.beta via neq: exit 1 => equal, 0 => differ
    r = subprocess.run([INTERP], input="%s\n(neq %s %s)\n" % (DEFS, g1, g2), capture_output=True, text=True)
    return "equal" if r.returncode == 1 else "differ"


checks = 0
fails = 0
i = 0
attempts = 0
while i < K and attempts < K * 6:
    attempts += 1
    e = gen(random.randint(1, 4))
    v = val(e)
    if v > 36:  # keep unary sizes (and the reducers' fuel) bounded
        continue
    i += 1
    ed = render(e, "z", "s", "p", "m")
    eg = render(e, "Ze", "Su", "plus", "mult")
    for rhs, expect in ((v, "equal"), (v + 1, "differ")):
        veq = eq_verdict(ed, unary(rhs, "z", "s"))
        vop = op_verdict(eg, unary(rhs, "Ze", "Su"))
        checks += 1
        if not (veq == vop == expect):
            fails += 1
            print("  FAIL  %s = %d : definitional=%s operational=%s expect=%s" % (ed[:48], rhs, veq, vop, expect))

print("seam fuzz (%d random Peano +/* expressions, %d checks): %d disagreements" % (i, checks, fails))
sys.exit(1 if fails else 0)
