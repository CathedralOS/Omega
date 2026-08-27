#!/usr/bin/env sh
# PREDICATE SOUNDNESS SEAM — the inductive predicates vs the reference interpreter.
#
# The soundness bridge has two operational seams already: semantics-diamond.sh checks
# the conversion rule (definitional `=`) against operational evaluation, and
# induction-soundness.sh checks inductively-proved UNIVERSALS against it. Both bridge
# the checker's logic to the gamma reference interpreter. The THIRD pillar — the
# inductive PREDICATES Mem / ProdIs / Perm, the relations the Fundamental Theorem of
# Arithmetic is built on — had no such bridge: predicate-diamond-fuzz cross-checks the
# three CHECKERS against each other, but nothing tied an accepted predicate proof to
# what the predicate OPERATIONALLY means. This closes that gap.
#
# For each predicate proof implementations/beta/check.beta accepts against a TRUE goal `(Rel R x… )`, the
# gamma interpreter must independently DECIDE the predicate (member / prod-equals /
# is-permutation, written as ordinary recursive gamma functions) and return 1 (holds).
# For each FALSE goal — the SAME proof aimed at a perturbed target — implementations/beta/check.beta must
# REJECT it AND the decision procedure must return 0. Two independent routes to "does
# this predicate hold" — a kernel typing derivation and an executable decision
# procedure — agreeing is evidence the checker's inductive rules are sound w.r.t. the
# reference interpreter (not a proof; the theorem is the open problem). Deterministic.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_PATH_BETA/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b implementations/beta/check.beta "$T/check.exe"            || { echo "build implementations/beta/check.beta failed"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }

# Decision procedures, as ordinary recursive gamma functions over the interpreter's
# Nat (Ze/Su) and List (Lnil/Lcons) ADTs. Each returns 1 (holds) / 0 (fails):
#   member  x l  : list membership          -> the operational twin of Mem    (Rel 777)
#   prod    l    : product of a list        -> compared by eqn to ProdIs's claim (Rel 778)
#   isperm  a b  : multiset equality (a is a permutation of b) -> twin of Perm (Rel 779)
DEFS='(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) (def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) (def eqn (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (eqn x y)) (w 0))))) (def member (x l) (match l (Lnil 0) ((Lcons h t) (if (eqn x h) 1 (member x t))))) (def prod (l) (match l (Lnil (Su Ze)) ((Lcons h t) (mult h (prod t))))) (def remove1 (x l) (match l (Lnil Lnil) ((Lcons h t) (if (eqn x h) t (Lcons h (remove1 x t)))))) (def isperm (a b) (match a (Lnil (match b (Lnil 1) (w 0))) ((Lcons h t) (if (member h b) (isperm t (remove1 h b)) 0))))'

PASS=0; FAIL=0
# pcase DESC  GOAL_TRUE  PROOF  GOAL_FALSE  DECISION_TRUE  DECISION_FALSE
# Two independent routes per case: implementations/beta/check.beta typing (accept GOAL_TRUE, reject GOAL_FALSE
# with the same PROOF) and the interpreter's decision procedure (1 on DECISION_TRUE, 0 on
# DECISION_FALSE). All four verdicts must line up or the case fails.
pcase() {
  vt=$(printf '%s %s' "$2" "$3" | "$T/check.exe")          # implementations/beta/check.beta on true goal
  vf=$(printf '%s %s' "$4" "$3" | "$T/check.exe")          # implementations/beta/check.beta on perturbed goal
  printf '%s\n%s\n' "$DEFS" "$5" | "$T/interp.exe" >/dev/null; dt=$?   # decision on true
  printf '%s\n%s\n' "$DEFS" "$6" | "$T/interp.exe" >/dev/null; df=$?   # decision on false
  if [ "$vt" = accept ] && [ "$vf" = reject ] && [ "$dt" = 1 ] && [ "$df" = 0 ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL $1 : check[true]=$vt check[false]=$vf decide[true]=$dt decide[false]=$df (want accept/reject/1/0)"
  fi
}

# --- Mem (Rel 777): 2 is a member of [1,2,3]; 5 is not ---
MEM_L='(cons (s z) (cons (s (s z)) (cons (s (s (s z))) nil)))'
MEM_LG='(Lcons (Su Ze) (Lcons (Su (Su Ze)) (Lcons (Su (Su (Su Ze))) Lnil)))'
pcase "Mem 2 in [1,2,3]" \
  "(Rel 777 (s (s z)) $MEM_L)" \
  "(memtail (s z) (memhead (s (s z)) (cons (s (s (s z))) nil)))" \
  "(Rel 777 (s (s (s (s (s z))))) $MEM_L)" \
  "(member (Su (Su Ze)) $MEM_LG)" \
  "(member (Su (Su (Su (Su (Su Ze))))) $MEM_LG)"

# --- ProdIs (Rel 778): product of [2,3] is 6, not 7 ---
PRD_L='(cons (s (s z)) (cons (s (s (s z))) nil))'
PRD_LG='(Lcons (Su (Su Ze)) (Lcons (Su (Su (Su Ze))) Lnil))'
PRD_6='(m (s (s z)) (m (s (s (s z))) (s z)))'
G6='(Su (Su (Su (Su (Su (Su Ze))))))'
G7='(Su (Su (Su (Su (Su (Su (Su Ze)))))))'
pcase "ProdIs [2,3]=6" \
  "(Rel 778 $PRD_L $PRD_6)" \
  "(pcons (s (s z)) (pcons (s (s (s z))) (pnil)))" \
  "(Rel 778 $PRD_L (s $PRD_6))" \
  "(eqn (prod $PRD_LG) $G6)" \
  "(eqn (prod $PRD_LG) $G7)"

# --- Perm (Rel 779) by a single swap: [1,2] ~ [2,1], but not ~ [2,5] ---
pcase "Perm [1,2]~[2,1] (swap)" \
  "(Rel 779 (cons (s z) (cons (s (s z)) nil)) (cons (s (s z)) (cons (s z) nil)))" \
  "(permswap (s z) (s (s z)) nil)" \
  "(Rel 779 (cons (s z) (cons (s (s z)) nil)) (cons (s (s z)) (cons (s (s (s (s (s z))))) nil)))" \
  "(isperm (Lcons (Su Ze) (Lcons (Su (Su Ze)) Lnil)) (Lcons (Su (Su Ze)) (Lcons (Su Ze) Lnil)))" \
  "(isperm (Lcons (Su Ze) (Lcons (Su (Su Ze)) Lnil)) (Lcons (Su (Su Ze)) (Lcons (Su (Su (Su (Su (Su Ze))))) Lnil)))"

# --- Perm (Rel 779) by permtrans of two real swaps: rotation [0,1,2] ~ [1,2,0] ---
# sw1: [0,1,2]~[1,0,2]; inner: [0,2]~[2,0]; sw2 = permskip 1 inner: [1,0,2]~[1,2,0].
ROT_SW1='(permswap z (s z) (cons (s (s z)) nil))'
ROT_SW2='(permskip (s z) (permswap z (s (s z)) nil))'
ROT_SRC='(cons z (cons (s z) (cons (s (s z)) nil)))'
ROT_DST='(cons (s z) (cons (s (s z)) (cons z nil)))'
ROT_BAD='(cons (s z) (cons (s (s z)) (cons (s (s (s (s (s z))))) nil)))'
ROT_AG='(Lcons Ze (Lcons (Su Ze) (Lcons (Su (Su Ze)) Lnil)))'
ROT_BG='(Lcons (Su Ze) (Lcons (Su (Su Ze)) (Lcons Ze Lnil)))'
ROT_BADG='(Lcons (Su Ze) (Lcons (Su (Su Ze)) (Lcons (Su (Su (Su (Su (Su Ze))))) Lnil)))'
pcase "Perm [0,1,2]~[1,2,0] (permtrans)" \
  "(Rel 779 $ROT_SRC $ROT_DST)" \
  "(permtrans $ROT_SW1 $ROT_SW2)" \
  "(Rel 779 $ROT_SRC $ROT_BAD)" \
  "(isperm $ROT_AG $ROT_BG)" \
  "(isperm $ROT_AG $ROT_BADG)"

echo "predicate soundness seam (inductive predicates vs operational decision): $PASS confirmed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
