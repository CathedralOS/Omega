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
DEFS='(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) (def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) (def eqn (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (eqn x y)) (w 0))))) (def append (a b) (match a (Lnil b) ((Lcons h t) (Lcons h (append t b))))) (def length (l) (match l (Lnil Ze) ((Lcons h t) (Su (length t))))) (def leq (a b) (match a (Lnil (match b (Lnil 1) (w 0))) ((Lcons h t) (match b ((Lcons i u) (if (eqn h i) (leq t u) 0)) (w 0))))) (def reverse (l) (match l (Lnil Lnil) ((Lcons h t) (append (reverse t) (Lcons h Lnil))))) (def suml (l) (match l (Lnil Ze) ((Lcons h t) (plus h (suml t))))) (def maps (l) (match l (Lnil Lnil) ((Lcons h t) (Lcons (Su h) (maps t)))))'

# concrete Nats and Lists for the operational instances
N1='(Su Ze)'; N2='(Su (Su Ze))'; N3='(Su (Su (Su Ze)))'; N4='(Su (Su (Su (Su Ze))))'
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

echo "soundness sweep (proved by check.beta AND true in the interpreter): $PASS confirmed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
