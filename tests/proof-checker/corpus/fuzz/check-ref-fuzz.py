#!/usr/bin/env python3
# check-ref-fuzz.py CHECK_EXE K — differential-test the trust anchor across FOUR proof categories: (1)
# propositional logic, (2) first-order (All/Exists), (3) equality by conversion (refl over Peano p/m), and
# (4) USER-FUNCTION arithmetic certificates — the actual translation-validation cert language (data/fun
# rules, (k ..) constructors, (f ..) applications). For K random valid certs, check.gamma and the independent
# reference check_ref.py must AGREE: both accept each cert, and both reject a perturbed (wrong-value/wrong-
# type) variant. So check_ref independently validates not just the checker's logic but the REAL TV certs.
# Deterministic (fixed seed). Curated induction (natind/listind/eqelim/disj/sinj), inductive-predicate
# (Mem/ProdIs/Perm), and named-lemma + generic-induction (def/use/rec) corpora are also cross-checked —
# check_ref now mirrors EVERY rule of check.gamma.
import sys, os, random, subprocess
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', '..', 'reference'))
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

def verdict_beta(cert):                                # cert = full input: <decls> <goal> <proof>
    payload = cert.encode() if isinstance(cert, str) else cert
    return subprocess.run([CHECK], input=payload, capture_output=True).stdout.decode().strip()

def verdict_ref(cert):
    return check_ref.check_input(cert)

def framed(source, tape, certificate):
    return (b'OMGCHK1\n' + len(source).to_bytes(8, 'little') + source +
            len(tape).to_bytes(8, 'little') + tape +
            len(certificate).to_bytes(8, 'little') + certificate)

# ---- user-function certificates (the translation-validation cert language) --------------------------
# user-Nat Z=(k 2), S x=(k 3 x), over uadd(21)/umul(23)/usub(22, monus)/upred(20) as delta user functions.
TVPRE = ("(data 2 0 0 0) (data 3 1 1 0) "
         "(fun 20 2 (k 2)) (fun 20 3 (v 0)) (fun 21 2 (y 0)) (fun 21 3 (k 3 (rec 0))) "
         "(fun 22 2 (y 0)) (fun 22 3 (f 20 (rec 0))) (fun 23 2 (k 2)) (fun 23 3 (f 21 (y 0) (rec 0)))")
def unat(k):
    t = "(k 2)"
    for _ in range(k):
        t = "(k 3 %s)" % t
    return t
def tv_expr(rng, depth):                               # -> (term_string, value); small trap-free values
    if depth <= 0 or rng.random() < 0.5:
        v = rng.randint(0, 4); return unat(v), v
    op = rng.choice(["+", "-", "*"])
    (ta, va), (tb, vb) = tv_expr(rng, depth - 1), tv_expr(rng, depth - 1)
    if op == "+":  return "(f 21 %s %s)" % (ta, tb), va + vb
    if op == "*":  return "(f 23 %s %s)" % (ta, tb), va * vb
    if va < vb:    va, vb, ta, tb = vb, va, tb, ta     # keep monus non-negative
    return "(f 22 %s %s)" % (tb, ta), va - vb          # a - b via (f 22 b a)

# ---- induction corpus (natind + listind + eqelim + disj + sinj) --------------------------------------
# Random valid induction proofs are impractical to generate, so this is a curated accept/reject corpus over
# the whole induction fragment — Nat AND list induction (nil/cons/app/len normalization): check.gamma and
# check_ref must agree on every one.
IND_CORPUS = [
    ("(All (= (p (v 0) z) (v 0))) (natind (= (p (v 0) z) (v 0)) (refl z) (gen (lam (= (p (v 0) z) (v 0)) (eqelim (= (s (p (v 1) z)) (s (v 0))) (hyp 0) (refl (s (p (v 0) z)))))))", "accept"),
    ("(All (= (m (v 0) z) z)) (natind (= (m (v 0) z) z) (refl z) (gen (lam (= (m (v 0) z) z) (hyp 0))))", "accept"),
    ("(-> (= z (s z)) (bot)) (lam (= z (s z)) (disj (hyp 0)))", "accept"),
    ("(-> (= (s (v 0)) (s z)) (= (v 0) z)) (lam (= (s (v 0)) (s z)) (sinj (hyp 0)))", "accept"),
    ("(All (-> (= (v 0) (s (v 0))) (bot))) (natind (-> (= (v 0) (s (v 0))) (bot)) (lam (= z (s z)) (disj (hyp 0))) (gen (lam (-> (= (v 0) (s (v 0))) (bot)) (lam (= (s (v 0)) (s (s (v 0)))) (app (hyp 1) (sinj (hyp 0)))))))", "accept"),
    ("(All (+ (= (v 0) z) (Exists (= (v 1) (s (v 0)))))) (natind (+ (= (v 0) z) (Exists (= (v 1) (s (v 0))))) (inl (Exists (= z (s (v 0)))) (refl z)) (gen (lam (+ (= (v 0) z) (Exists (= (v 1) (s (v 0))))) (inr (= (s (v 0)) z) (wit (= (s (v 1)) (s (v 0))) (v 0) (refl (s (v 0))))))))", "accept"),
    ("(All (All (-> (= (v 1) (v 0)) (= (s (v 1)) (s (v 0)))))) (gen (gen (lam (= (v 1) (v 0)) (eqelim (= (s (v 2)) (s (v 0))) (hyp 0) (refl (s (v 1)))))))", "accept"),
    # rejects: base is not P(0), and the fabrications guarded by disj/sinj's normal-form checks
    ("(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))", "reject"),
    ("(-> (= (s z) (s z)) (bot)) (lam (= (s z) (s z)) (disj (hyp 0)))", "reject"),
    ("(-> (= (s z) (s z)) (= z (s z))) (lam (= (s z) (s z)) (sinj (hyp 0)))", "reject"),
    # list induction: l++nil=l and append-associativity, via listind + Leibniz eqelim over cons/app; plus a
    # len-normalization equality and its perturbation
    ("(All (= (app (v 0) nil) (v 0))) (listind (= (app (v 0) nil) (v 0)) (refl nil) (gen (gen (lam (= (app (v 0) nil) (v 0)) (eqelim (= (cons (v 2) (app (v 1) nil)) (cons (v 2) (v 0))) (hyp 0) (refl (cons (v 1) (app (v 0) nil))))))))", "accept"),
    ("(All (All (All (= (app (app (v 0) (v 2)) (v 1)) (app (v 0) (app (v 2) (v 1))))))) (gen (gen (listind (= (app (app (v 0) (v 2)) (v 1)) (app (v 0) (app (v 2) (v 1)))) (refl (app (v 1) (v 0))) (gen (gen (lam (= (app (app (v 0) (v 3)) (v 2)) (app (v 0) (app (v 3) (v 2)))) (eqelim (= (cons (v 2) (app (app (v 1) (v 4)) (v 3))) (cons (v 2) (v 0))) (hyp 0) (refl (cons (v 1) (app (app (v 0) (v 3)) (v 2)))))))))))", "accept"),
    ("(= (len (cons z (cons z nil))) (s (s z))) (refl (s (s z)))", "accept"),
    ("(= (len (cons z (cons z nil))) (s z)) (refl (s z))", "reject"),
]

# ---- inductive-predicate corpus (Mem 777 / ProdIs 778 / Perm 779) ------------------------------------
# The relations the number-theory layer (FTA) is built on. Intro rules (memhead/tail, pnil/pcons, permnil/
# skip/swap/trans) and inversions (memcons/memnil, pnilinv/pconsinv) — check.gamma and check_ref must agree.
_ML = "(cons (s z) (cons (s (s z)) (cons (s (s (s z))) nil)))"       # [1,2,3]
_MPF = "(memtail (s z) (memhead (s (s z)) (cons (s (s (s z))) nil)))"  # Mem(2,[1,2,3])
_PL = "(cons (s (s z)) (cons (s (s (s z))) nil))"                      # [2,3]
_P6 = "(m (s (s z)) (m (s (s (s z))) (s z)))"                          # 2*(3*1)
_PPF = "(pcons (s (s z)) (pcons (s (s (s z))) (pnil)))"                # ProdIs([2,3],6)
_RS = "(cons z (cons (s z) (cons (s (s z)) nil)))"                     # [0,1,2]
_RD = "(cons (s z) (cons (s (s z)) (cons z nil)))"                     # [1,2,0]
_RSW1 = "(permswap z (s z) (cons (s (s z)) nil))"
_RSW2 = "(permskip (s z) (permswap z (s (s z)) nil))"
PRED_CORPUS = [
    ("(Rel 777 (s (s z)) %s) %s" % (_ML, _MPF), "accept"),
    ("(Rel 777 (s (s (s (s (s z))))) %s) %s" % (_ML, _MPF), "reject"),        # 5 ∉ [1,2,3]
    ("(Rel 778 %s %s) %s" % (_PL, _P6, _PPF), "accept"),
    ("(Rel 778 %s (s %s)) %s" % (_PL, _P6, _PPF), "reject"),                  # product ≠ 7
    ("(Rel 779 %s %s) (permtrans %s %s)" % (_RS, _RD, _RSW1, _RSW2), "accept"),
    ("(Rel 779 %s (cons (s z) (cons (s (s z)) (cons (s (s (s (s (s z))))) nil)))) (permtrans %s %s)" % (_RS, _RSW1, _RSW2), "reject"),
    ("(+ (= (s (s z)) (s z)) (Rel 777 (s (s z)) %s)) (memcons %s)" % (_PL, _MPF), "accept"),   # memcons inversion
    ("(-> (Rel 777 z nil) (bot)) (lam (Rel 777 z nil) (memnil (hyp 0)))", "accept"),          # memnil inversion
    ("(-> (Rel 777 z (cons z nil)) (bot)) (lam (Rel 777 z (cons z nil)) (memnil (hyp 0)))", "reject"),
    ("(= (m (s (s z)) (s z)) (s z)) (prodnilinv (pcons (s (s z)) (pnil)))", "reject"),   # ProdIs([2],2), nil-inv N/A
    ("(-> (Rel 778 nil (s (s z))) (= (s (s z)) (s z))) (lam (Rel 778 nil (s (s z))) (prodnilinv (hyp 0)))", "accept"),
    ("(Exists (& (= (m (s (s z)) (s z)) (m (s (s z)) (v 0))) (Rel 778 nil (v 0)))) (prodconsinv (pcons (s (s z)) (pnil)))", "accept"),
    ("(All (All (All (-> (Rel 778 (cons (v 2) (v 1)) (v 0)) (Exists (& (= (v 1) (m (v 3) (v 0))) (Rel 778 (v 2) (v 0)))))))) (gen (gen (gen (lam (Rel 778 (cons (v 2) (v 1)) (v 0)) (prodconsinv (hyp 0))))))", "accept"),
    ("(All (All (All (-> (Rel 778 (cons (v 2) (v 1)) (v 0)) (Exists (& (= (v 1) (m (v 0) (v 3))) (Rel 778 (v 2) (v 0)))))))) (gen (gen (gen (lam (Rel 778 (cons (v 2) (v 1)) (v 0)) (prodconsinv (hyp 0))))))", "reject"),
]

# ---- named-lemma (def/use) and generic structural-induction (rec) corpus ----------------------------
# (def N type proof) is verified up front then cited by (use N); a def that fails its stated type rejects
# the whole cert. (rec cidA cidB motive caseA caseB) is induction over any user datatype (con_case builds
# the per-constructor obligations incl. induction hypotheses from the (data ..) recursion flags).
_L0 = ("(def 0 (All (= (p (v 0) z) (v 0))) (natind (= (p (v 0) z) (v 0)) (refl z) "
       "(gen (lam (= (p (v 0) z) (v 0)) (eqelim (= (s (p (v 1) z)) (s (v 0))) (hyp 0) (refl (s (p (v 0) z))))))))")
LEMMA_CORPUS = [
    ("(def 0 (-> P P) (lam P (hyp 0))) (-> P P) (use 0)", "accept"),
    ("(def 0 (-> P Q) (lam P (hyp 0))) (-> P Q) (use 0)", "reject"),                  # def proof wrong type
    ("(def 0 (-> P P) (lam P (hyp 0))) (-> Q Q) (use 0)", "reject"),                  # cite doesn't match goal
    ("(def 0 (-> P P) (lam P (hyp 0))) (def 1 (-> (-> P P) (-> P P)) (lam (-> P P) (use 0))) (-> (-> P P) (-> P P)) (use 1)", "accept"),
    ("%s (All (= (m (s z) (v 0)) (v 0))) (gen (inst (use 0) (v 0)))" % _L0, "accept"),   # 1*n=n via lemma
    # rec: generic structural induction over a user Tree datatype (nil-leaf cid 0, binary-node cid 1)
    ("(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))", "accept"),
    ("(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))", "reject"),   # missing a0's IH
]

# Declaration tables are bounded and immutable before the first checked lemma.
# A later rewrite would otherwise change the definitional meaning of a lemma
# whose proof was already accepted.
DECL_CORPUS = [
    ("(data 2 0 0 0) (fun 7 2 z) (def 0 (= (f 7 (k 2)) z) (refl z)) (fun 7 2 (s z)) (= (f 7 (k 2)) z) (use 0)", "reject"),
    ("(data 2 0 0 0) (fun 7 2 z) (fun 7 2 z) (-> P P) (lam P (hyp 0))", "reject"),
    ("(fun 768 2 z) (-> P P) (lam P (hyp 0))", "reject"),
    ("(data 64 0 0 0) (-> P P) (lam P (hyp 0))", "reject"),
    ("(def 0 (-> P P) (lam P (hyp 0))) (def 0 (-> P P) (lam P (hyp 0))) (-> P P) (use 0)", "reject"),
    ("(-> P P) (lam P (hyp 0)) P", "reject"),
]

# D40 closed FloatMeaning terms. These cases pin the independent comparator to
# the same canonical correspondence tuple and carrier-specific proposition as
# the authoritative checker; no float evaluator or generic equality is added.
_FM32_NAN = '(fm 32 1 1 1 4 2143289345 0)'
_FM32_POS_ZERO = '(fm 32 1 1 1 4 0 0)'
_FM32_NEG_ZERO = '(fm 32 1 1 1 4 2147483648 0)'
_FM64_NAN = '(fm 64 2 2 1 4 0 2146959360)'
D40_CORPUS = [
    ("(FloatMeaningEqual %s %s) (fmrefl %s)" % (_FM32_NAN, _FM32_NAN, _FM32_NAN), 'accept'),
    ("(FloatMeaningEqual %s %s) (fmrefl %s)" % (_FM64_NAN, _FM64_NAN, _FM64_NAN), 'accept'),
    ("(FloatMeaningEqual %s %s) (fmrefl %s)" % (_FM32_POS_ZERO, _FM32_NEG_ZERO, _FM32_POS_ZERO), 'reject'),
    ("(-> (FloatMeaningEqual %s %s) (FloatMeaningEqual %s %s)) (lam (FloatMeaningEqual %s %s) (hyp 0))" %
     (_FM32_POS_ZERO, _FM32_NEG_ZERO, _FM32_POS_ZERO, _FM32_NEG_ZERO,
      _FM32_POS_ZERO, _FM32_NEG_ZERO), 'accept'),
    ("(FloatMeaningEqual (fm 32 1 9 1 4 0 0) (fm 32 1 9 1 4 0 0)) (fmrefl (fm 32 1 9 1 4 0 0))", 'reject'),
    ("(FloatMeaningEqual (fm 32 2 1 1 4 0 0) (fm 32 2 1 1 4 0 0)) (fmrefl (fm 32 2 1 1 4 0 0))", 'reject'),
    ("(FloatMeaningEqual (fm 32 1 1 2 4 0 0) (fm 32 1 1 2 4 0 0)) (fmrefl (fm 32 1 1 2 4 0 0))", 'reject'),
    ("(= %s %s) (refl %s)" % (_FM32_NAN, _FM32_NAN, _FM32_NAN), 'reject'),
    ("(FloatMeaningAlias %s %s) (fmrefl %s)" % (_FM32_NAN, _FM32_NAN, _FM32_NAN), 'reject'),
    ("(FloatMeaningEqual (fmalias 32 1 1 1 4 0 0) (fmalias 32 1 1 1 4 0 0)) (fmrefl (fmalias 32 1 1 1 4 0 0))", 'reject'),
    ("(FloatMeaningEqual %s %s) (fmreflalias %s)" % (_FM32_NAN, _FM32_NAN, _FM32_NAN), 'reject'),
]

FRAME_CORPUS = [
    (framed(b'abc', b'abc', b'(= source tape) (refl source)'), 'accept'),
    (framed(b'abc', b'abd', b'(= source tape) (refl source)'), 'reject'),
    (framed(b'abc', b'x', b'(fun 100 61 z) (fun 100 62 (s z)) (fun 100 63 (p (rec 0) (rec 1))) (= (f 100 source) (s (s (s z)))) (refl (s (s (s z))))'), 'accept'),
    (framed(b'abc', b'abc', b'(data 60 2 0 0) (= source tape) (refl source)'), 'reject'),
    (b'(= source source) (refl source)', 'reject'),
]

fails = 0; n = 0
for cert, expect in IND_CORPUS + PRED_CORPUS + LEMMA_CORPUS + DECL_CORPUS + D40_CORPUS + FRAME_CORPUS:
    n += 1
    vb, vr = verdict_beta(cert), verdict_ref(cert)
    if not (vb == vr == expect):
        fails += 1
        print("  FAIL (corpus) beta=%s ref=%s want=%s : %s" % (vb, vr, expect, cert[:160]))
for _ in range(K):
    r = random.random()
    if r < 0.35:                                       # propositional
        a, b, c = random.sample(range(6), 3)
        goal, proof = random.choice(prop_schemas(a, b, c))
        cases = [("%s %s" % (pr(goal), pf(proof)), "accept")]
        m = mutate(goal, 7)
        if m: cases.append(("%s %s" % (pr(m), pf(proof)), "reject"))
    elif r < 0.6:                                      # first-order (quantifiers)
        goal, proof = random.choice(quant_schemas(random.randrange(3), 5, gterm(random.randint(0, 2))))
        cases = [("%s %s" % (pr(goal), pf(proof)), "accept")]
        m = mutate(goal, 7)
        if m: cases.append(("%s %s" % (pr(m), pf(proof)), "reject"))
    elif r < 0.8:                                      # equality / conversion (refl over Peano p/m)
        e = arith(random.Random(random.random()), 3); v = tval(e)
        cases = [("%s %s" % (pr(("eq", e, num(v))), pf(("refl", num(v)))), "accept"),
                 ("%s %s" % (pr(("eq", e, num(v + 1))), pf(("refl", num(v)))), "reject")]
    else:                                              # user-function arithmetic certs (TV cert language)
        tvterm, v = tv_expr(random.Random(random.random()), 3)
        cases = [("%s (= %s %s) (refl %s)" % (TVPRE, tvterm, unat(v), unat(v)), "accept"),
                 ("%s (= %s %s) (refl %s)" % (TVPRE, tvterm, unat(v + 1), unat(v + 1)), "reject")]
    for cert, expect in cases:
        n += 1
        vb, vr = verdict_beta(cert), verdict_ref(cert)
        if not (vb == vr == expect):
            fails += 1
            print("  FAIL beta=%s ref=%s want=%s : %s" % (vb, vr, expect, cert[:160]))
print("%d ok, %d failed" % (n - fails, fails))
sys.exit(1 if fails else 0)
