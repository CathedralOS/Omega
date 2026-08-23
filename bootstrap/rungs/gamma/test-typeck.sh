#!/usr/bin/env sh
# Gate for the Gamma static type checker (typeck.beta). Compiled by bc, run on the
# seed. Well-typed programs -> exit 1; type errors -> exit 0.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd "${OMEGA_PATH_BETA_COMPILER_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
"${OMEGA_PATH_BETA_COMPILER_RUST}"/build/bc.exe < typeck.beta > "$T/tc.asm" || { echo "bc(typeck.beta) failed"; exit 1; }
"$ASM" < "$T/tc.asm" > "$T/tc.tape" || { echo "assemble failed"; exit 1; }
stamp_seed "$T/tc.tape" "$SEED" "$T/tc.exe" >/dev/null 2>&1

PASS=0; FAIL=0
tc() { # program  expect(1 ok / 0 type-error)  desc
  printf '%s' "$1" | "$T/tc.exe"; got=$?
  if [ "$got" = "$2" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL want $2 got $got : $3"; fi
}
# phase 1 — Int + typed functions
tc '(def add ((a Int) (b Int)) Int (+ a b)) (def main () Int (add 2 3))' 1 'well-typed'
tc '(def id ((x Int)) Int x)' 1 'identity'
tc '(def f ((a Int) (b Int)) Int (if (lt a b) a b))' 1 'if/branches'
tc '(def f ((a Int)) Int (let y (+ a 1) (* y y)))' 1 'let'
tc '(def f ((a Int)) Int (g a)) (def g ((x Int)) Int x)' 1 'forward call'
tc '(def add ((a Int) (b Int)) Int (+ a b)) (def main () Int (add 2))' 0 'arity too few'
tc '(def add ((a Int) (b Int)) Int (+ a b)) (def main () Int (add 1 2 3))' 0 'arity too many'
tc '(def main () Int (nope 1))' 0 'unknown function'
# phase 2 — data declarations (ADTs) + match, well-typed
tc '(data Nat (Z) (S Nat)) (def pred ((n Nat)) Nat (match n (Z Z) ((S m) m))) (def main () Nat (pred (S (S Z))))' 1 'Nat pred'
tc '(data List (Nil) (Cons Int List)) (def len ((xs List)) Int (match xs (Nil 0) ((Cons h t) (+ 1 (len t)))))' 1 'list length'
tc '(data Nat (Z) (S Nat)) (def plus ((a Nat) (b Nat)) Nat (match a (Z b) ((S m) (S (plus m b)))))' 1 'Nat plus'
# phase 2 — TYPE ERRORS
tc '(data List (Nil) (Cons Int List)) (def bad ((xs List)) Int (+ xs 1))' 0 'Int op on a List'
tc '(data List (Nil) (Cons Int List)) (def bad () List (Cons Nil Nil))' 0 'Cons wants Int got List'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n (Z 0) ((S m) m)))' 0 'match arms differ'
tc '(data Nat (Z) (S Nat)) (data List (Nil) (Cons Int List)) (def bad ((n Nat)) Int (match n (Nil 0) (x 1)))' 0 'Nil pattern on a Nat'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (+ n 1))' 0 'return Nat but body Int'
# phase 2 — CONSTRUCTOR application and pattern arity (distinct from call arity above)
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (S (S n)))' 1 'control: nested constructor ok'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (S Z Z))' 0 'constructor too many args'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (S))' 0 'constructor too few args'
tc '(data Nat (Z) (S Nat)) (def bad ((n Int)) Nat (S n))' 0 'constructor arg wrong type (S on Int)'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Nat (Nope n))' 0 'unknown constructor'
tc '(data Pair (Mk Int Int)) (def bad ((p Pair)) Int (match p ((Mk a) a)))' 0 'pattern arity wrong (1 of 2)'
# the proof kernel's OWN code is statically type-safe under gamma's type system
printf '%s' "$(cat "${OMEGA_PATH_PROOF_KERNEL}"/implementations/gamma/checker_typed.gamma)" | "$T/tc.exe"; ct=$?
if [ "$ct" = 1 ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL checker_typed.gamma should be well-typed (got $ct)"; fi
echo "gamma typeck: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
