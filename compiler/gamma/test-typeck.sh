#!/usr/bin/env sh
# Gate for the Gamma static type checker (typeck.beta). Compiled by bc, run on the
# seed. Well-typed programs -> exit 1; type errors -> exit 0.
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
../beta-lang-rs/build/bc.exe < typeck.beta > "$T/tc.asm" || { echo "bc(typeck.beta) failed"; exit 1; }
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
echo "gamma typeck: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
