#!/usr/bin/env python3
# Random broad coverage of the checker DIAMOND on the INDUCTIVE PREDICATES -- Mem (Rel 777, list
# membership), ProdIs (Rel 778, list product) and Perm (Rel 779, list permutation), the three inductive
# relations that underpin the Fundamental Theorem of Arithmetic. seam-fuzz covers the reducer,
# checker-diamond-fuzz the equality conversion, logic-diamond-fuzz the first-order logic; this covers the
# last checker subsystem otherwise cross-checked across all three checkers only at the ~25 hand-picked
# checker-diamond.sh cases. It builds random valid intro-proofs (memhead/memtail chains, pnil/pcons chains,
# permnil/permskip/permswap/permtrans) and requires all three trust-anchor checkers -- check.beta,
# checker.gamma, checker_typed.gamma -- to ACCEPT each proof against its true predicate goal AND REJECT it
# against a perturbed goal. A single disagreement is a bug in one checker's inductive rules. Deterministic.
import sys, random, subprocess

CHECK, INTERP, CGAMMA = sys.argv[1], sys.argv[2], sys.argv[3]
CTYPED = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4] else None  # type-erased checker_typed.gamma
K = int(sys.argv[5]) if len(sys.argv) > 5 else 120
CGDEFS = open(CGAMMA).read()
CTDEFS = open(CTYPED).read() if CTYPED else None
random.seed(20240607)

# token maps: check.beta (lowercase) vs checker.gamma (capitalized). Rel N is identical in both.
B = {"s": "s", "z": "z", "cons": "cons", "nil": "nil", "memhead": "memhead", "memtail": "memtail",
     "pnil": "pnil", "pcons": "pcons", "permnil": "permnil", "permskip": "permskip",
     "permswap": "permswap", "permtrans": "permtrans"}
G = {"s": "Su", "z": "Ze", "cons": "Lcons", "nil": "Lnil", "memhead": "MemHead", "memtail": "MemTail",
     "pnil": "Pnil", "pcons": "Pcons", "permnil": "Permnil", "permskip": "Permskip",
     "permswap": "Permswap", "permtrans": "Permtrans"}


def nat(n, T):
    s = T["z"]
    for _ in range(n):
        s = "(%s %s)" % (T["s"], s)
    return s


def lst(xs, T):
    s = T["nil"]
    for x in reversed(xs):
        s = "(%s %s %s)" % (T["cons"], nat(x, T), s)
    return s


def prod_term(xs, T):  # the product as a nested multiplication ending in 1 (what pcons builds)
    s = nat(1, T)
    for x in reversed(xs):
        s = "(m %s %s)" % (nat(x, T), s) if T is B else "(Mu %s %s)" % (nat(x, T), s)
    return s


def mem_case():
    L = [random.randint(0, 4) for _ in range(random.randint(1, 4))]
    k = random.randrange(len(L))
    x = L[k]
    def proof(T):
        p = "(%s %s %s)" % (T["memhead"], nat(x, T), lst(L[k + 1:], T))
        for j in range(k - 1, -1, -1):
            p = "(%s %s %s)" % (T["memtail"], nat(L[j], T), p)
        return p
    xbad = max(L) + 1  # not a member of L
    goal = lambda v, T: "(Rel 777 %s %s)" % (nat(v, T), lst(L, T))
    return goal(x, B), goal(x, G), proof(B), proof(G), goal(xbad, B), goal(xbad, G)


def prod_case():
    L = [random.randint(1, 3) for _ in range(random.randint(0, 4))]  # avoid 0 so the product is informative
    def proof(T):
        p = "(%s)" % T["pnil"]
        for x in reversed(L):
            p = "(%s %s %s)" % (T["pcons"], nat(x, T), p)
        return p
    goal = lambda pt, T: "(Rel 778 %s %s)" % (lst(L, T), pt)
    badB = "(s %s)" % prod_term(L, B)  # product + 1: a false ProdIs
    badG = "(Su %s)" % prod_term(L, G)
    return goal(prod_term(L, B), B), goal(prod_term(L, G), G), proof(B), proof(G), goal(badB, B), goal(badG, G)


def perm_case():
    shape = random.choice(("id", "swap", "skipswap", "transid", "compose"))
    a, b, c = random.randint(0, 4), random.randint(0, 4), random.randint(0, 4)
    fresh = max(a, b, c) + 1  # not among the elements -> any target containing it is NOT a permutation
    if shape == "id":  # Perm(L, L) by a permskip chain over permnil
        L = [a, b, c][: random.randint(1, 3)]
        def proof(T):
            p = "(%s)" % T["permnil"]
            for x in reversed(L):
                p = "(%s %s %s)" % (T["permskip"], nat(x, T), p)
            return p
        src, dst, bad = L, L, L[:-1] + [fresh]
    elif shape == "swap":  # Perm([a,b]++r, [b,a]++r) by permswap
        r = [c] if random.random() < 0.5 else []
        def proof(T):
            return "(%s %s %s %s)" % (T["permswap"], nat(a, T), nat(b, T), lst(r, T))
        src, dst, bad = [a, b] + r, [b, a] + r, [fresh, a] + r
    elif shape == "skipswap":  # permskip c over a swap
        def proof(T):
            sw = "(%s %s %s %s)" % (T["permswap"], nat(a, T), nat(b, T), T["nil"])
            return "(%s %s %s)" % (T["permskip"], nat(c, T), sw)
        src, dst, bad = [c, a, b], [c, b, a], [c, b, fresh]
    elif shape == "transid":  # permtrans of a swap and its inverse -> identity on [a,b]
        def proof(T):
            sw1 = "(%s %s %s %s)" % (T["permswap"], nat(a, T), nat(b, T), T["nil"])
            sw2 = "(%s %s %s %s)" % (T["permswap"], nat(b, T), nat(a, T), T["nil"])
            return "(%s %s %s)" % (T["permtrans"], sw1, sw2)
        src, dst, bad = [a, b], [a, b], [a, fresh]
    else:  # compose: a genuine 2-transposition rotation [a,b,c] -> [b,c,a] via permtrans of two REAL swaps
        def proof(T):
            sw1 = "(%s %s %s %s)" % (T["permswap"], nat(a, T), nat(b, T), lst([c], T))   # [a,b,c] ~ [b,a,c]
            inner = "(%s %s %s %s)" % (T["permswap"], nat(a, T), nat(c, T), T["nil"])      # [a,c] ~ [c,a]
            sw2 = "(%s %s %s)" % (T["permskip"], nat(b, T), inner)                         # [b,a,c] ~ [b,c,a]
            return "(%s %s %s)" % (T["permtrans"], sw1, sw2)                               # [a,b,c] ~ [b,c,a]
        src, dst, bad = [a, b, c], [b, c, a], [b, c, fresh]
    goal = lambda d, T: "(Rel 779 %s %s)" % (lst(src, T), lst(d, T))
    return goal(dst, B), goal(dst, G), proof(B), proof(G), goal(bad, B), goal(bad, G)


def make_case():
    return random.choice((mem_case, prod_case, perm_case))()


def beta_verdict(goal, proof):
    return subprocess.run([CHECK], input="%s %s" % (goal, proof), capture_output=True, text=True).stdout.strip()


def gamma_verdict(defs, check_expr):
    r = subprocess.run([INTERP], input="%s\n%s\n" % (defs, check_expr), capture_output=True, text=True)
    return "accept" if r.returncode == 1 else "reject"


checks = 0
fails = 0
for i in range(K):
    gb, gg, pb, pg, bb, bg = make_case()
    for goal_b, goal_g, expect in ((gb, gg, "accept"), (bb, bg, "reject")):
        if goal_b is None:
            continue
        vb = beta_verdict(goal_b, pb)
        gexpr = "(check %s %s)" % (pg, goal_g)
        vg = gamma_verdict(CGDEFS, gexpr)
        vt = gamma_verdict(CTDEFS, gexpr) if CTDEFS else expect
        checks += 1
        if not (vb == vg == vt == expect):
            fails += 1
            print("  FAIL %s : check.beta=%s checker.gamma=%s typed=%s expect=%s"
                  % (goal_g[:46], vb, vg, vt, expect))

oracles = "check.beta + checker.gamma" + (" + checker_typed.gamma" if CTDEFS else "")
print("predicate-diamond fuzz (%d random Mem/ProdIs/Perm proofs, %d checks, oracles: %s): %d disagreements"
      % (K, checks, oracles, fails))
sys.exit(1 if fails else 0)
