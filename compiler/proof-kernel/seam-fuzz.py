#!/usr/bin/env python3
# Random broad coverage of the proof/meaning soundness seam across the checker's reduction paths: built-in
# Peano arithmetic {z, s, +, *, length}, built-in Lists {nil, cons, append}, AND USER-FUNCTION recursion
# (the `fun`/`rec` machinery -- the most complex and bug-prone reducer): add/mult over a user-Nat, plus
# length/suml/reverse over a user LIST type (the corpus's own (k 2)/(k 3 h t) encoding, reverse nesting a
# call to a user append) -- recursion over a 2-field constructor. For each generated case the DEFINITIONAL
# equality (eq.beta: normalize both sides) and the reference interpreter's OPERATIONAL evaluation
# (interp.beta running gamma's own plus/mult/append/length) must AGREE: both that `E = value(E)`
# (verdict "equal") and that they differ from a perturbation (verdict "differ"). Nat results compare
# with neq, List results with leq. A single disagreement is a soundness break at the seam. Deterministic
# (fixed seed), so it is reproducible.
import sys, random, subprocess

EQ, INTERP = sys.argv[1], sys.argv[2]
K = int(sys.argv[3]) if len(sys.argv) > 3 else 120
DEFS = (
    '(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) '
    '(def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) '
    '(def neq (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (neq x y)) (w 0))))) '
    '(def append (a b) (match a (Lnil b) ((Lcons h t) (Lcons h (append t b))))) '
    '(def length (l) (match l (Lnil Ze) ((Lcons h t) (Su (length t))))) '
    '(def leq (a b) (match a (Lnil (match b (Lnil 1) (w 0))) ((Lcons h t) (match b ((Lcons i u) (if (neq h i) (leq t u) 0)) (w 0))))) '
    '(def suml (l) (match l (Lnil Ze) ((Lcons h t) (plus h (suml t))))) '
    '(def reverse (l) (match l (Lnil Lnil) ((Lcons h t) (append (reverse t) (Lcons h Lnil)))))'
)
# user-function add (fid 10) and mult (fid 11) over a user-Nat (Z = (k 2), S x = (k 3 x)) -- the same
# fun/rec encoding the semantics-diamond uses, exercised here at random arguments.
MFUN = "(fun 10 2 (y 0)) (fun 10 3 (k 3 (rec 0))) (fun 11 2 (k 2)) (fun 11 3 (f 10 (y 0) (rec 0)))"
# user-list FUNCTIONS over a user list type (nil' = (k 2), cons' h t = (k 3 h t)) -- the corpus's own
# encoding (reverse-append.elab etc.). eq.beta infers the recursive arg from (rec 1), so NO data decl is
# needed. length = fun 10, suml = fun 11 (raw plus is `p`, not `+`), reverse = fun 9 via append = fun 8.
# This exercises the deepest reducer path: recursion over a 2-field user constructor + a nested fun call.
UL_LEN = "(fun 10 2 z) (fun 10 3 (s (rec 1)))"
UL_SUM = "(fun 11 2 z) (fun 11 3 (p (v 0) (rec 1)))"
UL_REV = "(fun 8 2 (y 0)) (fun 8 3 (k 3 (v 0) (rec 1))) (fun 9 2 (k 2)) (fun 9 3 (f 8 (rec 1) (k 3 (v 0) (k 2))))"
random.seed(20240607)  # deterministic -- reproducible cases every run

KERNEL = {"z": "z", "s": "s", "+": "p", "*": "m", "len": "len", "nil": "nil", "cons": "cons", "app": "app"}
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


def un(n):  # user-Nat: Z = (k 2), S x = (k 3 x)
    out = "(k 2)"
    for _ in range(n):
        out = "(k 3 %s)" % out
    return out


def ulist(lst):  # user-list: nil' = (k 2), cons' h t = (k 3 h t); heads are built-in KERNEL nats
    out = "(k 2)"
    for x in reversed(lst):
        out = "(k 3 %s %s)" % (rnat(x, KERNEL), out)
    return out


def eq_verdict(d1, d2):
    return subprocess.run([EQ], input="%s %s" % (d1, d2), capture_output=True, text=True).stdout.strip()


def op_verdict(rel, g1, g2):  # rel = "neq" (Nat) or "leq" (List); exit 1 => equal
    r = subprocess.run([INTERP], input="%s\n(%s %s %s)\n" % (DEFS, rel, g1, g2), capture_output=True, text=True)
    return "equal" if r.returncode == 1 else "differ"


# Build a case: (ed, eg, same_d, diff_d, same_g, diff_g, rel). ed/eg are the expression in the checker
# and interpreter syntaxes; same/diff are the matching and perturbed right-hand sides in each.
def make_case():
    r = random.random()
    if r < 0.22:  # USER-Nat function recursion (fun/rec): add (fid 10) / mult (fid 11)
        op = random.choice(("add", "mult"))
        a, b = random.randint(0, 4), random.randint(0, 4)
        v = a + b if op == "add" else a * b
        cid, gop = ("10", "plus") if op == "add" else ("11", "mult")
        ed = "%s (f %s %s %s)" % (MFUN, cid, un(a), un(b))  # checker: fun defs + the call
        eg = "(%s %s %s)" % (gop, rnat(a, GAMMA), rnat(b, GAMMA))  # interp: Peano op
        return ed, eg, un(v), un(v + 1), rnat(v, GAMMA), rnat(v + 1, GAMMA), "neq"
    if r < 0.44:  # USER-LIST function recursion (fun/rec over a 2-field user list): length/suml/reverse
        lst = [random.randint(0, 3) for _ in range(random.randint(0, 4))]
        lu, lg = ulist(lst), rlist(lst, GAMMA)
        op = random.choice(("len", "sum", "rev"))
        if op == "len":
            v = len(lst)
            return ("%s (f 10 %s)" % (UL_LEN, lu), "(length %s)" % lg,
                    rnat(v, KERNEL), rnat(v + 1, KERNEL), rnat(v, GAMMA), rnat(v + 1, GAMMA), "neq")
        if op == "sum":
            v = sum(lst)
            return ("%s (f 11 %s)" % (UL_SUM, lu), "(suml %s)" % lg,
                    rnat(v, KERNEL), rnat(v + 1, KERNEL), rnat(v, GAMMA), rnat(v + 1, GAMMA), "neq")
        rev = list(reversed(lst))  # reverse: list-valued, compared with leq / perturbed by prepending 0
        return ("%s (f 9 %s)" % (UL_REV, lu), "(reverse %s)" % lg,
                ulist(rev), ulist([0] + rev), rlist(rev, GAMMA), rlist([0] + rev, GAMMA), "leq")
    if r < 0.66:  # built-in List path
        e = gen_list(random.randint(1, 4))
        v = val_list(e)
        if len(v) > 8 or any(x > 8 for x in v):
            return None
        return (render(e, KERNEL), render(e, GAMMA),
                rlist(v, KERNEL), rlist([0] + v, KERNEL), rlist(v, GAMMA), rlist([0] + v, GAMMA), "leq")
    # built-in Nat path
    e = gen_nat(random.randint(1, 4))
    v = val_nat(e)
    if v > 36:
        return None
    return (render(e, KERNEL), render(e, GAMMA),
            rnat(v, KERNEL), rnat(v + 1, KERNEL), rnat(v, GAMMA), rnat(v + 1, GAMMA), "neq")


checks = 0
fails = 0
i = 0
attempts = 0
while i < K and attempts < K * 8:
    attempts += 1
    case = make_case()
    if case is None:
        continue
    i += 1
    ed, eg, same_d, diff_d, same_g, diff_g, rel = case
    for rhs_d, rhs_g, expect in ((same_d, same_g, "equal"), (diff_d, diff_g, "differ")):
        veq = eq_verdict(ed, rhs_d)
        vop = op_verdict(rel, eg, rhs_g)
        checks += 1
        if not (veq == vop == expect):
            fails += 1
            print("  FAIL  %s vs %s : definitional=%s operational=%s expect=%s" % (ed[:46], rhs_d[:24], veq, vop, expect))

print("seam fuzz (%d random cases over +/*/length/append AND user-function add/mult/length/suml/reverse, %d checks): %d disagreements" % (i, checks, fails))
sys.exit(1 if fails else 0)
