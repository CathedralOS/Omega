#!/usr/bin/env sh
# SOUNDNESS SWEEP — proved-in-Delta AND true-in-the-interpreter, across the corpus.
#
# delta.md's honest-edge #1 is the soundness theorem (provable-in-Delta => true-about-
# execution per the reference interpreter). semantics-diamond.sh exhibits the seam for
# definitional equalities and induction-soundness.sh for a handful of inductive
# universals. This widens that EVIDENCE across the proof corpus: for each curated
# theorem that is an equation over functions the gamma reference interpreter can
# evaluate, it does BOTH, independently:
#   (1) elaborate proofs/NAME.elab and require the TRUSTED check.beta to ACCEPT it
#       (the universal is proved), and
#   (2) evaluate the SAME equation at concrete instances in the interpreter and
#       require it to HOLD.
# If the checker ever proved a universal the interpreter refutes at a concrete point,
# that is a soundness break at the gamma/delta seam, and this catches it. Not the
# theorem — bounded, broad evidence for it. The proof side is sourced straight from the
# corpus (elaborate-and-check), so adding a theorem is one line, not a hand-built cert.
# Needs python3 (the elaborator), like elab-test.sh; skips cleanly without it.
cd "$(dirname "$0")"
if ! command -v python3 >/dev/null 2>&1; then
  echo "soundness sweep: skipped (python3 absent)"; exit 0
fi
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta "$T/check.exe"            || { echo "build check.beta failed"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }

# gamma operational twins of the corpus functions (the same defs the other seam scripts
# use): + * (Nat) with structural eq; append/reverse/sum/map (List) with structural eq.
DEFS='(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) (def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) (def eqn (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (eqn x y)) (w 0))))) (def append (a b) (match a (Lnil b) ((Lcons h t) (Lcons h (append t b))))) (def length (l) (match l (Lnil Ze) ((Lcons h t) (Su (length t))))) (def leq (a b) (match a (Lnil (match b (Lnil 1) (w 0))) ((Lcons h t) (match b ((Lcons i u) (if (eqn h i) (leq t u) 0)) (w 0))))) (def reverse (l) (match l (Lnil Lnil) ((Lcons h t) (append (reverse t) (Lcons h Lnil))))) (def suml (l) (match l (Lnil Ze) ((Lcons h t) (plus h (suml t))))) (def maps (l) (match l (Lnil Lnil) ((Lcons h t) (Lcons (Su h) (maps t))))) (def nle (a b) (match a (Ze 1) ((Su x) (match b (Ze 0) ((Su y) (nle x y)))))) (def even (n) (match n (Ze 1) ((Su x) (match x (Ze 0) ((Su y) (even y)))))) (def odd (n) (match n (Ze 0) ((Su x) (match x (Ze 1) ((Su y) (odd y)))))) (def band (a b) (if a b 0)) (def bnot (a) (if a 0 1)) (def eqb (a b) (if a b (bnot b))) (def dvds (a b m) (if (nle (mult m a) b) (if (eqn (mult m a) b) 1 (dvds a b (Su m))) 0)) (def dvd (a b) (match a (Ze (eqn b Ze)) (w (dvds a b Ze)))) (def monus (a b) (match b (Ze a) ((Su y) (match a (Ze Ze) ((Su x) (monus x y)))))) (def nmodk (a m) (if (nle m a) (nmodk (monus a m) m) a)) (def nmod (a m) (match m (Ze a) (w (nmodk a m)))) (def modeq (a b m) (eqn (nmod a m) (nmod b m)))'
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

PASS=0; FAIL=0
# sweep DESC  ELAB_BASENAME  OP_EXPR(closed gamma expr that must evaluate to 1)
sweep() {
  v=$(python3 elab.py --check "$T/check.exe" < "proofs/$2.elab" 2>&1)
  if [ "$v" != accept ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : check.beta did not accept proofs/$2.elab ($v)"; return; fi
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

# --- List equational universals (structural list eq via leq, Nat eq via eqn) ---
sweep "(a++b)++c = a++(b++c)"   append-assoc-user \
  "(leq (append (append $LA $LB) $LC) (append $LA (append $LB $LC)))"
sweep "rev(a++b) = rev b ++ rev a" reverse-append \
  "(leq (reverse (append $LA $LB)) (append (reverse $LB) (reverse $LA)))"
sweep "sum(a++b) = sum a + sum b"  sum-append \
  "(if (eqn (suml (append $LA $LB)) (plus (suml $LA) (suml $LB))) (eqn (suml (append $LC $LA)) (plus (suml $LC) (suml $LA))) 0)"
sweep "map(a++b) = map a ++ map b" map-append \
  "(leq (maps (append $LA $LB)) (append (maps $LA) (maps $LB)))"

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
sweep "a<=b & c<=d -> a+c<=b+d" le-add-both \
  "(if (band (nle $N2 $N4) (nle $N3 $N5)) (nle (plus $N2 $N3) (plus $N4 $N5)) 0)"
sweep "a<b -> c+a < c+b"        lt-add-left \
  "(if (nle (Su $N2) $N5) (nle (Su (plus $N3 $N2)) (plus $N3 $N5)) 0)"

echo "soundness sweep (proved by check.beta AND true in the interpreter): $PASS confirmed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
