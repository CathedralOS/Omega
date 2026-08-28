#!/usr/bin/env sh
# SOUNDNESS SWEEP — kernel-accepted AND true-in-the-interpreter, across the corpus.
#
# proof_kernel.md's honest-edge #1 is the soundness theorem (kernel-accepted => true-about-
# execution per the reference interpreter). semantics-diamond.sh exhibits the seam for
# definitional equalities and induction-soundness.sh for a handful of inductive
# universals. This widens that EVIDENCE across the proof corpus: for each curated
# theorem that is an equation over functions the gamma reference interpreter can
# evaluate, it does BOTH, independently:
#   (1) elaborate corpus/proofs/NAME.elab and require the TRUSTED implementations/beta/check.beta to ACCEPT it
#       (the universal is proved), and
#   (2) evaluate the SAME equation at concrete instances in the interpreter and
#       require it to HOLD.
# If the checker ever proved a universal the interpreter refutes at a concrete point,
# that is a soundness break at the proof/meaning seam, and this catches it. Not the
# theorem — bounded, broad evidence for it. The proof side is sourced straight from the
# corpus (elaborate-and-check), so adding a theorem is one line, not a hand-built cert.
# Needs python3 (the elaborator), like elab-test.sh; skips cleanly without it.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "lattice paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" || exit $?
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" || exit $?
. "$OMEGA_PATH_ALPHA_CHECKER/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_ALPHA_CHECKER"
if ! command -v python3 >/dev/null 2>&1; then
  echo "soundness sweep: skipped (python3 absent)"; exit 0
fi
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }

# gamma operational twins of the corpus functions (the same defs the other seam scripts
# use): + * (Nat) with structural eq; append/reverse/sum/map (List) with structural eq.
DEFS='(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) (def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) (def eqn (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (eqn x y)) (w 0))))) (def append (a b) (match a (Lnil b) ((Lcons h t) (Lcons h (append t b))))) (def length (l) (match l (Lnil Ze) ((Lcons h t) (Su (length t))))) (def leq (a b) (match a (Lnil (match b (Lnil 1) (w 0))) ((Lcons h t) (match b ((Lcons i u) (if (eqn h i) (leq t u) 0)) (w 0))))) (def reverse (l) (match l (Lnil Lnil) ((Lcons h t) (append (reverse t) (Lcons h Lnil))))) (def suml (l) (match l (Lnil Ze) ((Lcons h t) (plus h (suml t))))) (def maps (l) (match l (Lnil Lnil) ((Lcons h t) (Lcons (Su h) (maps t))))) (def prod (l) (match l (Lnil (Su Ze)) ((Lcons h t) (mult h (prod t))))) (def nle (a b) (match a (Ze 1) ((Su x) (match b (Ze 0) ((Su y) (nle x y)))))) (def even (n) (match n (Ze 1) ((Su x) (match x (Ze 0) ((Su y) (even y)))))) (def odd (n) (match n (Ze 0) ((Su x) (match x (Ze 1) ((Su y) (odd y)))))) (def band (a b) (if a b 0)) (def bnot (a) (if a 0 1)) (def eqb (a b) (if a b (bnot b))) (def dvds (a b m) (if (nle (mult m a) b) (if (eqn (mult m a) b) 1 (dvds a b (Su m))) 0)) (def dvd (a b) (match a (Ze (eqn b Ze)) (w (dvds a b Ze)))) (def monus (a b) (match b (Ze a) ((Su y) (match a (Ze Ze) ((Su x) (monus x y)))))) (def nmodk (a m) (if (nle m a) (nmodk (monus a m) m) a)) (def nmod (a m) (match m (Ze a) (w (nmodk a m)))) (def modeq (a b m) (eqn (nmod a m) (nmod b m))) (def pow (b e) (match e (Ze (Su Ze)) ((Su k) (mult b (pow b k))))) (def size (t) (match t (Leaf (Su Ze)) ((Node l r) (plus (size l) (size r))))) (def mirror (t) (match t (Leaf Leaf) ((Node l r) (Node (mirror r) (mirror l))))) (def teq (a b) (match a (Leaf (match b (Leaf 1) (w 0))) ((Node l r) (match b ((Node p q) (if (teq l p) (teq r q) 0)) (w 0))))) (def emirror (t) (match t ((ETip v) (ETip v)) ((EBranch l r) (EBranch (emirror r) (emirror l))))) (def eflatten (t) (match t ((ETip v) (Lcons v Lnil)) ((EBranch l r) (append (eflatten l) (eflatten r))))) (def ecount (t) (match t ((ETip v) (Su Ze)) ((EBranch l r) (plus (ecount l) (ecount r))))) (def enodes (t) (match t ((ETip v) Ze) ((EBranch l r) (Su (plus (enodes l) (enodes r)))))) (def intadd (x y) (match x ((Pair a b) (match y ((Pair c d) (Pair (plus a c) (plus b d))))))) (def intmul (x y) (match x ((Pair a b) (match y ((Pair c d) (Pair (plus (mult a c) (mult b d)) (plus (mult a d) (mult b c)))))))) (def intneg (x) (match x ((Pair a b) (Pair b a)))) (def inteq (x y) (match x ((Pair a b) (match y ((Pair c d) (eqn (plus a d) (plus b c))))))) (def intle (x y) (match x ((Pair a b) (match y ((Pair c d) (nle (plus a d) (plus c b)))))))'
# nle/even/odd are INDEPENDENT structural twins of the corpus predicates (le encoded as
# (ex d. a+d=b); even as (ex k. n=k+k); odd as (ex k. n=s(k+k))) — so "even n xor odd n"
# is a fact the interpreter computes, not a tautology of the defs. dvd decides the divides
# relation (a|b = ex m. m*a=b) by bounded search on the quotient m while m*a<=b — total
# because m*a strictly grows for a>=1; (0|b) iff b=0.

# concrete Nats and Lists for the operational instances
N1='(Su Ze)'; N2='(Su (Su Ze))'; N3='(Su (Su (Su Ze)))'; N4='(Su (Su (Su (Su Ze))))'
N5='(Su (Su (Su (Su (Su Ze)))))'; N6='(Su (Su (Su (Su (Su (Su Ze))))))'
N7='(Su (Su (Su (Su (Su (Su (Su Ze)))))))'; N8='(Su (Su (Su (Su (Su (Su (Su (Su Ze))))))))'
LA='(Lcons Ze (Lcons (Su Ze) Lnil))'                 # [0,1]
LB='(Lcons (Su (Su Ze)) Lnil)'                       # [2]
LC='(Lcons (Su (Su (Su Ze))) (Lcons Ze Lnil))'       # [3,0]
LD='(Lcons (Su (Su Ze)) (Lcons (Su (Su (Su Ze))) Lnil))'  # [2,3] (zero-free, for product theorems)
# binary trees: TA/TB have valueless leaves (Leaf/Node, for size+mirror); E1/E2 are expression trees with
# valued leaves (ETip v / EBranch l r, for flatten/count/nodes).
TA='(Node Leaf (Node Leaf Leaf))'
TB='(Node (Node Leaf Leaf) (Node (Node Leaf Leaf) Leaf))'
E1='(EBranch (ETip (Su Ze)) (EBranch (ETip (Su (Su Ze))) (ETip (Su (Su (Su Ze))))))'
E2='(EBranch (EBranch (ETip Ze) (ETip (Su Ze))) (ETip (Su (Su Ze))))'
# integers as difference pairs (Pair a b ~ a-b); ~ is `inteq`: (a,b)~(c,d) iff a+d=b+c.
P1='(Pair (Su (Su (Su Ze))) (Su Ze))'              # 3-1 = 2
P2='(Pair (Su Ze) (Su (Su (Su (Su Ze)))))'         # 1-4 = -3
P3='(Pair (Su (Su Ze)) Ze)'                        # 2-0 = 2 (a different rep of 2)
P4='(Pair (Su (Su (Su (Su Ze)))) (Su (Su Ze)))'    # 4-2 = 2 (yet another rep of 2, for transitivity)
Z0='(Pair Ze Ze)'; ON='(Pair (Su Ze) Ze)'          # 0 and 1

PASS=0; FAIL=0
# sweep DESC  ELAB_BASENAME  OP_EXPR(closed gamma expr that must evaluate to 1)
sweep() {
  v=$(python3 tools/elab.py --check "$T/check.exe" < "corpus/proofs/$2.elab" 2>&1)
  if [ "$v" != accept ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : implementations/beta/check.beta did not accept corpus/proofs/$2.elab ($v)"; return; fi
  printf '%s\n%s\n' "$DEFS" "$3" | "$T/interp.exe" >/dev/null; n=$?
  if [ "$n" != 1 ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : interpreter refutes the equation at the instances (got $n)"; return; fi
  PASS=$((PASS+1))
}

# --- Nat equational universals (checked at two concrete points each) ---
sweep "a+b = b+a"               add-commutes \
  "(if (eqn (plus $N2 $N3) (plus $N3 $N2)) (eqn (plus $N1 $N4) (plus $N4 $N1)) 0)"
sweep "a+(s b) = s(a+b)"        add-succ-right \
  "(if (eqn (plus $N2 (Su $N3)) (Su (plus $N2 $N3))) (eqn (plus $N3 (Su $N1)) (Su (plus $N3 $N1))) 0)"
sweep "(a*b)*c = a*(b*c)"       mult-assoc \
  "(if (eqn (mult (mult $N2 $N3) $N2) (mult $N2 (mult $N3 $N2))) (eqn (mult (mult $N3 $N1) $N2) (mult $N3 (mult $N1 $N2))) 0)"
sweep "a*b = b*a"               mult-comm \
  "(if (eqn (mult $N2 $N3) (mult $N3 $N2)) (eqn (mult $N1 $N4) (mult $N4 $N1)) 0)"
sweep "a*(b*c) = b*(a*c)"       mul-swap-mid \
  "(if (eqn (mult $N2 (mult $N3 $N1)) (mult $N3 (mult $N2 $N1))) (eqn (mult $N1 (mult $N2 $N3)) (mult $N2 (mult $N1 $N3))) 0)"
sweep "n*(n+1) is even"         consecutive-product-even \
  "(if (even (mult $N2 (Su $N2))) (even (mult $N3 (Su $N3))) 0)"
# --- parity alternation (computed via the even/odd structural twins; tested where the antecedent holds) ---
sweep "even n -> odd(s n)"      succ-of-even-is-odd \
  "(if (odd (Su $N2)) (odd (Su $N4)) 0)"
sweep "odd n -> even(s n)"      succ-of-odd-is-even \
  "(if (even (Su $N3)) (even (Su $N5)) 0)"
sweep "even n OR even(s n)"     consecutive-even \
  "(if (if (even $N3) 1 (even (Su $N3))) (if (even $N4) 1 (even (Su $N4))) 0)"

# --- List equational universals (structural list eq via leq, Nat eq via eqn) ---
sweep "(a++b)++c = a++(b++c)"   append-assoc-user \
  "(leq (append (append $LA $LB) $LC) (append $LA (append $LB $LC)))"
sweep "rev(a++b) = rev b ++ rev a" reverse-append \
  "(leq (reverse (append $LA $LB)) (append (reverse $LB) (reverse $LA)))"
sweep "sum(a++b) = sum a + sum b"  sum-append \
  "(if (eqn (suml (append $LA $LB)) (plus (suml $LA) (suml $LB))) (eqn (suml (append $LC $LA)) (plus (suml $LC) (suml $LA))) 0)"
sweep "map(a++b) = map a ++ map b" map-append \
  "(leq (maps (append $LA $LB)) (append (maps $LA) (maps $LB)))"
sweep "a ++ nil = a"                append-right-id-user \
  "(if (leq (append $LA Lnil) $LA) (leq (append $LC Lnil) $LC) 0)"
sweep "sum(map a) = sum a + length a" sum-map \
  "(if (eqn (suml (maps $LA)) (plus (suml $LA) (length $LA))) (eqn (suml (maps $LC)) (plus (suml $LC) (length $LC))) 0)"
# --- reverse structural family (reverse is an involution; commutes with map; preserves sum) ---
sweep "reverse(reverse a) = a"          reverse-involution \
  "(if (leq (reverse (reverse $LA)) $LA) (leq (reverse (reverse $LC)) $LC) 0)"
sweep "reverse(map a) = map(reverse a)" rev-map-commute \
  "(if (leq (reverse (maps $LA)) (maps (reverse $LA))) (leq (reverse (maps $LC)) (maps (reverse $LC))) 0)"
sweep "sum(reverse a) = sum a"          sum-reverse \
  "(if (eqn (suml (reverse $LA)) (suml $LA)) (eqn (suml (reverse $LC)) (suml $LC)) 0)"
# --- product structural family (product over a zero-free list LD=[2,3]; empty product = 1) ---
sweep "product(reverse a) = product a"  product-reverse \
  "(if (eqn (prod (reverse $LD)) (prod $LD)) (eqn (prod (reverse $LB)) (prod $LB)) 0)"
sweep "product(a++b) = product a * product b" product-append \
  "(if (eqn (prod (append $LD $LB)) (mult (prod $LD) (prod $LB))) (eqn (prod (append $LB $LD)) (mult (prod $LB) (prod $LD))) 0)"
sweep "product(nil) = 1"                 product-one-list-nil \
  "(eqn (prod Lnil) (Su Ze))"

# --- Relational / predicate universals (le = ex d. a+d=b ; parity via even/odd twins).
# For implications, the instances are chosen to satisfy the hypotheses, and the
# decidable conclusion must hold; for the parity laws, every n in range must comply. ---
sweep "a <= a"                  le-refl \
  "(if (nle Ze Ze) (if (nle $N2 $N2) (nle $N4 $N4) 0) 0)"
sweep "a<=b & b<=c -> a<=c"     le-trans \
  "(if (band (nle $N2 $N3) (nle $N3 $N4)) (nle $N2 $N4) 0)"
sweep "a<=b -> s a <= s b"      le-succ-mono \
  "(if (nle $N2 $N3) (nle (Su $N2) (Su $N3)) 0)"
sweep "not (even n and odd n)"  parity-exclusive \
  "(if (bnot (band (even Ze) (odd Ze))) (if (bnot (band (even $N3) (odd $N3))) (bnot (band (even $N4) (odd $N4))) 0) 0)"
sweep "odd n  <->  not even n"  odd-iff-not-even \
  "(if (eqb (odd $N1) (bnot (even $N1))) (if (eqb (odd $N2) (bnot (even $N2))) (eqb (odd $N3) (bnot (even $N3))) 0) 0)"
sweep "even a -> even (a*b)"    even-mult \
  "(if (even (mult $N2 $N3)) (even (mult $N4 $N3)) 0)"
sweep "n + n = 2 * n"           double-is-twice \
  "(if (eqn (plus $N2 $N2) (mult $N2 $N2)) (eqn (plus $N3 $N3) (mult $N2 $N3)) 0)"

# --- the divides relation (a|b = ex m. m*a=b) — backbone of the prime/FTA corpus.
# dvd is an independent decidable twin; implications check the conclusion at instances
# that satisfy the hypotheses. ---
sweep "a | a"                   divides-refl \
  "(if (dvd $N3 $N3) (dvd $N4 $N4) 0)"
sweep "a|b & b|c -> a|c"        divides-trans \
  "(if (band (dvd $N2 $N4) (dvd $N4 $N8)) (dvd $N2 $N8) 0)"
sweep "a|b & a|c -> a|(b+c)"    divides-add \
  "(if (band (dvd $N2 $N4) (dvd $N2 $N6)) (dvd $N2 (plus $N4 $N6)) 0)"
sweep "a|b -> a|(c*b)"          divides-mult \
  "(if (dvd $N2 $N4) (dvd $N2 (mult $N3 $N4)) 0)"
sweep "a|b -> a|(b*b)"          divides-square \
  "(if (dvd $N2 $N4) (dvd $N2 (mult $N4 $N4)) 0)"

# --- the congruence relation (a == b (mod m), encoded ex j k. a+m*k = m*j+b) — the
# arithmetic the FTA / modular proofs run on. modeq is an independent decidable twin
# (equal remainders via nmod); implications check the conclusion where the hyp holds. ---
sweep "a == a (mod m)"          mod-refl \
  "(if (modeq $N7 $N7 $N3) (modeq $N5 $N5 $N2) 0)"
sweep "a==b -> b==a (mod m)"    mod-sym \
  "(if (modeq $N7 $N1 $N3) (modeq $N1 $N7 $N3) 0)"
sweep "a==b & b==c -> a==c"     mod-trans \
  "(if (band (modeq $N7 $N1 $N3) (modeq $N1 $N4 $N3)) (modeq $N7 $N4 $N3) 0)"
sweep "a==b -> a+c == b+c"      mod-add-compat \
  "(if (modeq $N7 $N1 $N3) (modeq (plus $N7 $N5) (plus $N1 $N5) $N3) 0)"
sweep "a==b -> a*c == b*c"      mod-mul-compat \
  "(if (modeq $N7 $N1 $N3) (modeq (mult $N7 $N2) (mult $N1 $N2) $N3) 0)"

# --- the lemmas the CAPSTONE proofs rest on (sqrt2-irrational, FTA): parity of squares,
# odd products, divisibility of products, and order monotonicity. Cross-checking these
# against execution is the soundness evidence that matters most for the headline proofs. ---
sweep "even(a*a) -> even a"     even-square-even \
  "(if (band (even (mult $N2 $N2)) (even (mult $N6 $N6))) (band (even $N2) (even $N6)) 0)"
sweep "odd a & odd b -> odd a*b" odd-mult \
  "(if (band (odd $N3) (odd $N5)) (odd (mult $N3 $N5)) 0)"
sweep "a|b & c|d -> (a*c)|(b*d)" divides-products \
  "(if (band (dvd $N2 $N4) (dvd $N3 $N6)) (dvd (mult $N2 $N3) (mult $N4 $N6)) 0)"
sweep "d|(x+y) & d|x -> d|y"     divides-sub \
  "(if (band (dvd $N2 (plus $N4 $N6)) (dvd $N2 $N4)) (dvd $N2 $N6) 0)"
sweep "ProdIs(L,n) & Mem(x,L) & 0<n -> x<=n" element-le-product \
  "(if (nle $N2 (prod $LD)) (nle $N3 (prod $LD)) 0)"
sweep "a<=b & c<=d -> a+c<=b+d" le-add-both \
  "(if (band (nle $N2 $N4) (nle $N3 $N5)) (nle (plus $N2 $N3) (plus $N4 $N5)) 0)"
sweep "a <= a+b"                le-add-upper \
  "(if (nle $N2 (plus $N2 $N3)) (nle $N3 (plus $N3 $N1)) 0)"
sweep "a<b -> c+a < c+b"        lt-add-left \
  "(if (nle (Su $N2) $N5) (nle (Su (plus $N3 $N2)) (plus $N3 $N5)) 0)"
sweep "a<=B & b<=C -> a*b<=B*C" mult-bound \
  "(if (band (nle $N2 $N3) (nle $N2 $N4)) (nle (mult $N2 $N2) (mult $N3 $N4)) 0)"
sweep "0<c & a<b -> c*a < c*b"  lt-mult-mono-left \
  "(if (band (nle $N1 $N2) (nle (Su $N1) $N3)) (nle (Su (mult $N2 $N1)) (mult $N2 $N3)) 0)"

# --- More divisibility / congruence / parity (dvd, modeq, even twins) ---
sweep "a | 0"                   divides-zero \
  "(if (dvd $N2 Ze) (if (dvd $N3 Ze) (dvd $N5 Ze) 0) 0)"
sweep "1 | a"                   divides-one \
  "(if (dvd $N1 $N2) (if (dvd $N1 $N3) (dvd $N1 $N5) 0) 0)"
sweep "a==b & c==d -> a+c==b+d (mod m)"  mod-add-two-sided \
  "(if (band (modeq $N2 $N5 $N3) (modeq $N1 $N4 $N3)) (modeq (plus $N2 $N1) (plus $N5 $N4) $N3) 0)"
sweep "a==b & c==d -> a*c==b*d (mod m)"  mod-mul-two-sided \
  "(if (band (modeq $N2 $N5 $N3) (modeq $N1 $N4 $N3)) (modeq (mult $N2 $N1) (mult $N5 $N4) $N3) 0)"
sweep "even n  <->  2 | n"      even-iff-two-divides \
  "(if (eqb (even $N4) (dvd $N2 $N4)) (if (eqb (even $N5) (dvd $N2 $N5)) (eqb (even Ze) (dvd $N2 Ze)) 0) 0)"

# --- EXPONENTIATION family (pow twin: b^0=1, b^(Sk)=b*b^k) — the laws the omega growth proofs lean on ---
sweep "a^(m+n) = a^m * a^n"     power-add \
  "(if (eqn (pow $N2 (plus $N2 $N1)) (mult (pow $N2 $N2) (pow $N2 $N1))) (eqn (pow $N3 (plus $N1 $N2)) (mult (pow $N3 $N1) (pow $N3 $N2))) 0)"
sweep "(a*b)^k = a^k * b^k"     power-mul-distrib \
  "(if (eqn (pow (mult $N2 $N3) $N2) (mult (pow $N2 $N2) (pow $N3 $N2))) (eqn (pow (mult $N2 $N2) $N2) (mult (pow $N2 $N2) (pow $N2 $N2))) 0)"
sweep "(a^m)^n = a^(m*n)"       power-mul-exponent \
  "(if (eqn (pow (pow $N2 $N2) $N2) (pow $N2 (mult $N2 $N2))) (eqn (pow (pow $N3 $N1) $N2) (pow $N3 (mult $N1 $N2))) 0)"
sweep "0<a -> 0<a^n"            power-positive \
  "(if (nle $N1 (pow $N2 $N3)) (nle $N1 (pow $N3 $N2)) 0)"
sweep "a|b -> a^n | b^n"        divides-power \
  "(if (dvd $N2 $N4) (dvd (pow $N2 $N2) (pow $N4 $N2)) 0)"
sweep "even a -> even a^(S k)"  even-power \
  "(if (even $N2) (even (pow $N2 (Su $N1))) 0)"
# cheap cross-family completions (existing twins, no new machinery): finish the power family + fill gaps.
sweep "a<=b -> a^n <= b^n"      power-monotone \
  "(if (nle $N2 $N3) (nle (pow $N2 $N2) (pow $N3 $N2)) 0)"
sweep "odd a -> odd a^n"        odd-power \
  "(if (odd $N3) (odd (pow $N3 $N2)) 0)"
sweep "n | n^(S k)"             divides-own-power \
  "(dvd $N3 (pow $N3 (Su $N1)))"
sweep "a+b<=c -> a<=c"          le-summand-bound \
  "(if (nle (plus $N2 $N3) $N6) (nle $N2 $N6) 0)"
sweep "a|b & b|a -> a=b"        divides-antisym \
  "(if (band (dvd $N4 $N4) (dvd $N4 $N4)) (eqn $N4 $N4) 0)"
sweep "a=b -> a==b (mod n)"     mod-eq-cong \
  "(if (eqn $N5 $N5) (modeq $N5 $N5 $N3) 0)"
sweep "len(a++b) = len(b++a)"   append-length-comm \
  "(if (eqn (length (append $LA $LB)) (length (append $LB $LA))) (eqn (length (append $LB $LA)) (length (append $LA $LB))) 0)"

# --- BINARY TREE family (a NEW ADT in the sweep) — twins: size/mirror/teq over Leaf|Node, and
# emirror/eflatten/ecount/enodes over expression trees ETip|EBranch. Mirror is an involution that
# preserves size/count/nodes and reverses the leaf sequence; a full tree has one more leaf than node. ---
sweep "mirror(mirror t) = t"         tree-mirror-involution \
  "(if (teq (mirror (mirror $TA)) $TA) (teq (mirror (mirror $TB)) $TB) 0)"
sweep "size(mirror t) = size t"      tree-mirror-size \
  "(if (eqn (size (mirror $TA)) (size $TA)) (eqn (size (mirror $TB)) (size $TB)) 0)"
sweep "count e = len(flatten e)"     count-flatten-len \
  "(if (eqn (ecount $E1) (length (eflatten $E1))) (eqn (ecount $E2) (length (eflatten $E2))) 0)"
sweep "flatten(mirror e) = rev(flatten e)" flatten-mirror-rev \
  "(if (leq (eflatten (emirror $E1)) (reverse (eflatten $E1))) (leq (eflatten (emirror $E2)) (reverse (eflatten $E2))) 0)"
sweep "count(mirror e) = count e"    count-mirror-preserved \
  "(if (eqn (ecount (emirror $E1)) (ecount $E1)) (eqn (ecount (emirror $E2)) (ecount $E2)) 0)"
sweep "nodes(mirror e) = nodes e"    nodes-mirror-preserved \
  "(if (eqn (enodes (emirror $E1)) (enodes $E1)) (eqn (enodes (emirror $E2)) (enodes $E2)) 0)"
sweep "count e = s(nodes e)"         leaves-internal-plus-one \
  "(if (eqn (ecount $E1) (Su (enodes $E1))) (eqn (ecount $E2) (Su (enodes $E2))) 0)"

# --- INTEGER RING family (Z built as difference pairs, equality up to ~) — the ring axioms and the
# equivalence laws, cross-checked at concrete signed points. Completes the sweep's ADT coverage:
# nats -> lists -> trees -> integers. Twins intadd/intmul/intneg/inteq over Pair a b. ---
sweep "x+y ~ y+x  (int)"             int-add-comm \
  "(if (inteq (intadd $P1 $P2) (intadd $P2 $P1)) (inteq (intadd $P2 $P3) (intadd $P3 $P2)) 0)"
sweep "(x+y)+z ~ x+(y+z)  (int)"     int-add-assoc \
  "(if (inteq (intadd (intadd $P1 $P2) $P3) (intadd $P1 (intadd $P2 $P3))) (inteq (intadd (intadd $P2 $P3) $P1) (intadd $P2 (intadd $P3 $P1))) 0)"
sweep "x+0 ~ x  (int)"               int-add-identity \
  "(if (inteq (intadd $P1 $Z0) $P1) (inteq (intadd $P2 $Z0) $P2) 0)"
sweep "x+(-x) ~ 0  (int)"            int-add-inverse \
  "(if (inteq (intadd $P1 (intneg $P1)) $Z0) (inteq (intadd $P2 (intneg $P2)) $Z0) 0)"
sweep "x*y ~ y*x  (int)"             int-mul-comm \
  "(if (inteq (intmul $P1 $P2) (intmul $P2 $P1)) (inteq (intmul $P2 $P3) (intmul $P3 $P2)) 0)"
sweep "x*(y+z) ~ x*y+x*z  (int)"     int-mul-distrib \
  "(if (inteq (intmul $P1 (intadd $P2 $P3)) (intadd (intmul $P1 $P2) (intmul $P1 $P3))) (inteq (intmul $P2 (intadd $P3 $P1)) (intadd (intmul $P2 $P3) (intmul $P2 $P1))) 0)"
sweep "x*1 ~ x  (int)"               int-mul-identity \
  "(if (inteq (intmul $P1 $ON) $P1) (inteq (intmul $P2 $ON) $P2) 0)"
sweep "x ~ x  (int)"                 int-eq-refl \
  "(if (inteq $P1 $P1) (inteq $P2 $P2) 0)"
sweep "x~y -> y~x  (int)"            int-eq-sym \
  "(if (inteq $P1 $P3) (inteq $P3 $P1) 0)"
sweep "x~y & y~z -> x~z  (int)"      int-eq-trans \
  "(if (band (inteq $P1 $P3) (inteq $P3 $P4)) (inteq $P1 $P4) 0)"
# ZZ ORDER laws (intle: (a,b)<=(c,d) iff a+d<=c+b) — completes the integer family (ring axioms + order).
sweep "x <= x  (int)"               int-le-refl \
  "(if (intle $P1 $P1) (intle $P2 $P2) 0)"
sweep "x<=y & y<=z -> x<=z  (int)"   int-le-trans \
  "(if (band (intle $P2 $P1) (intle $P1 $P3)) (intle $P2 $P3) 0)"
sweep "x<=y & y<=x -> x~y  (int)"    int-le-antisym \
  "(if (band (intle $P1 $P3) (intle $P3 $P1)) (inteq $P1 $P3) 0)"
sweep "x<=y or y<=x  (int)"          int-le-total \
  "(if (intle $P1 $P2) 1 (intle $P2 $P1))"
sweep "m<=n -> iota m <= iota n"     int-le-from-nat \
  "(if (nle $N2 $N3) (intle (Pair $N2 Ze) (Pair $N3 Ze)) 0)"
sweep "iota m <= iota n -> m<=n"     int-le-to-nat \
  "(if (intle (Pair $N2 Ze) (Pair $N3 Ze)) (nle $N2 $N3) 0)"

# --- List LENGTH structural universals (length, append, reverse, maps twins; Nat eq via eqn) ---
sweep "length(a++b) = length a + length b"  len-append-user \
  "(if (eqn (length (append $LA $LB)) (plus (length $LA) (length $LB))) (eqn (length (append $LC $LA)) (plus (length $LC) (length $LA))) 0)"
sweep "length(reverse a) = length a"        len-reverse \
  "(if (eqn (length (reverse $LA)) (length $LA)) (eqn (length (reverse $LC)) (length $LC)) 0)"
sweep "length(map a) = length a"            map-length \
  "(if (eqn (length (maps $LA)) (length $LA)) (eqn (length (maps $LC)) (length $LC)) 0)"

echo "soundness sweep (proved by implementations/beta/check.beta AND true in the interpreter): $PASS confirmed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
