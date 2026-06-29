#!/usr/bin/env python3
# PREDICATE SOUNDNESS FUZZER -- broad random coverage of the predicate-soundness SEAM (the curated
# predicate-soundness.sh is the few hand-picked cases). It bridges the inductive predicates Mem (Rel 777),
# ProdIs (Rel 778) and Perm (Rel 779) -- the FTA's foundation -- to the gamma reference interpreter, one
# level beyond predicate-diamond-fuzz: that fuzzer cross-checks the three CHECKERS against each other;
# THIS one cross-checks a kernel typing derivation against an independent EXECUTABLE decision procedure
# (member / prod+eqn / isperm-by-multiset-equality, ordinary recursive gamma functions). For each random
# case it builds a valid intro-proof and requires (1) check.beta ACCEPTS it against the true goal and
# REJECTS the same proof against a perturbed goal, AND (2) the interpreter's decision procedure returns 1
# on the true instance and 0 on the perturbed one. A disagreement is either a checker bug in the inductive
# rules or a soundness gap between the kernel and operational meaning. Deterministic. Needs python3.
import sys, random, subprocess

CHECK, INTERP = sys.argv[1], sys.argv[2]
K = int(sys.argv[3]) if len(sys.argv) > 3 else 80
random.seed(20240608)

# decision procedures over the interpreter's Nat (Ze/Su) and List (Lnil/Lcons) ADTs; each returns 1/0.
DEFS = ('(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) '
        '(def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) '
        '(def eqn (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (eqn x y)) (w 0))))) '
        '(def member (x l) (match l (Lnil 0) ((Lcons h t) (if (eqn x h) 1 (member x t))))) '
        '(def prod (l) (match l (Lnil (Su Ze)) ((Lcons h t) (mult h (prod t))))) '
        '(def remove1 (x l) (match l (Lnil Lnil) ((Lcons h t) (if (eqn x h) t (Lcons h (remove1 x t)))))) '
        '(def isperm (a b) (match a (Lnil (match b (Lnil 1) (w 0))) '
        '((Lcons h t) (if (member h b) (isperm t (remove1 h b)) 0))))')


def natB(n):  # check.beta: (s (s ... z))
    s = "z"
    for _ in range(n):
        s = "(s %s)" % s
    return s


def natG(n):  # interpreter: (Su (Su ... Ze))
    s = "Ze"
    for _ in range(n):
        s = "(Su %s)" % s
    return s


def lstB(xs):
    s = "nil"
    for x in reversed(xs):
        s = "(cons %s %s)" % (natB(x), s)
    return s


def lstG(xs):
    s = "Lnil"
    for x in reversed(xs):
        s = "(Lcons %s %s)" % (natG(x), s)
    return s


def prodtermB(xs):  # the product as nested multiplication ending in 1 -- what pcons builds
    s = natB(1)
    for x in reversed(xs):
        s = "(m %s %s)" % (natB(x), s)
    return s


# each case returns (goal_true, proof, goal_bad, dec_true, dec_bad): check.beta strings + interpreter exprs
def mem_case():
    L = [random.randint(0, 4) for _ in range(random.randint(1, 4))]
    k = random.randrange(len(L))
    x = L[k]
    p = "(memhead %s %s)" % (natB(x), lstB(L[k + 1:]))
    for j in range(k - 1, -1, -1):
        p = "(memtail %s %s)" % (natB(L[j]), p)
    xbad = max(L) + 1  # not a member
    return ("(Rel 777 %s %s)" % (natB(x), lstB(L)), p, "(Rel 777 %s %s)" % (natB(xbad), lstB(L)),
            "(member %s %s)" % (natG(x), lstG(L)), "(member %s %s)" % (natG(xbad), lstG(L)))


def prod_case():
    while True:
        L = [random.randint(1, 3) for _ in range(random.randint(0, 3))]  # >=1 so product is informative
        pv = 1
        for x in L:
            pv *= x
        if pv <= 20:  # keep the unary product small enough for the fuel-bounded interpreter
            break
    p = "(pnil)"
    for x in reversed(L):
        p = "(pcons %s %s)" % (natB(x), p)
    return ("(Rel 778 %s %s)" % (lstB(L), prodtermB(L)), p, "(Rel 778 %s (s %s))" % (lstB(L), prodtermB(L)),
            "(eqn (prod %s) %s)" % (lstG(L), natG(pv)), "(eqn (prod %s) %s)" % (lstG(L), natG(pv + 1)))


def perm_case():
    shape = random.choice(("id", "swap", "skipswap", "transid", "compose"))
    a, b, c = random.randint(0, 4), random.randint(0, 4), random.randint(0, 4)
    fresh = max(a, b, c) + 1
    if shape == "id":
        L = [a, b]
        p = "(permskip %s (permskip %s (permnil)))" % (natB(a), natB(b))
        src, dst, bad = [a, b], [a, b], [a, fresh]
    elif shape == "swap":
        p = "(permswap %s %s nil)" % (natB(a), natB(b))
        src, dst, bad = [a, b], [b, a], [b, fresh]
    elif shape == "skipswap":
        sw = "(permswap %s %s nil)" % (natB(a), natB(b))
        p = "(permskip %s %s)" % (natB(c), sw)
        src, dst, bad = [c, a, b], [c, b, a], [c, b, fresh]
    elif shape == "transid":
        sw1 = "(permswap %s %s nil)" % (natB(a), natB(b))
        sw2 = "(permswap %s %s nil)" % (natB(b), natB(a))
        p = "(permtrans %s %s)" % (sw1, sw2)
        src, dst, bad = [a, b], [a, b], [a, fresh]
    else:  # compose: a genuine 2-transposition rotation [a,b,c] -> [b,c,a]
        sw1 = "(permswap %s %s %s)" % (natB(a), natB(b), lstB([c]))
        inner = "(permswap %s %s nil)" % (natB(a), natB(c))
        sw2 = "(permskip %s %s)" % (natB(b), inner)
        p = "(permtrans %s %s)" % (sw1, sw2)
        src, dst, bad = [a, b, c], [b, c, a], [b, c, fresh]
    return ("(Rel 779 %s %s)" % (lstB(src), lstB(dst)), p, "(Rel 779 %s %s)" % (lstB(src), lstB(bad)),
            "(isperm %s %s)" % (lstG(src), lstG(dst)), "(isperm %s %s)" % (lstG(src), lstG(bad)))


def check(goal, proof):
    return subprocess.run([CHECK], input="%s %s" % (goal, proof), capture_output=True, text=True).stdout.strip()


def decide(expr):
    return subprocess.run([INTERP], input="%s\n%s\n" % (DEFS, expr), capture_output=True, text=True).returncode


checks = 0
fails = 0
for i in range(K):
    gt, proof, gf, dt, df = random.choice((mem_case, prod_case, perm_case))()
    vt, vf = check(gt, proof), check(gf, proof)
    et, ef = decide(dt), decide(df)
    checks += 1
    if not (vt == "accept" and vf == "reject" and et == 1 and ef == 0):
        fails += 1
        print("  FAIL %s : check[true]=%s check[bad]=%s decide[true]=%s decide[bad]=%s (want accept/reject/1/0)"
              % (gt[:46], vt, vf, et, ef))

print("predicate-soundness fuzz (%d random Mem/ProdIs/Perm goals, kernel derivation vs operational "
      "decision): %d disagreements" % (checks, fails))
sys.exit(1 if fails else 0)
