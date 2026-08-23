#!/usr/bin/env sh
# Gate for the Gamma reference interpreter (interp.beta, stage 1). Compiles it
# with bc (the self-hosting Rust-free Beta compiler), then evaluates Gamma
# programs and checks the integer result (the process exit code).
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
. "$OMEGA_PATH_BETA/artifact_env.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null

"$T/bc.exe" < interp.beta > "$T/g.asm" || { echo "bc(interp.beta) failed"; exit 1; }
"$ASM" < "$T/g.asm" > "$T/g.tape" || { echo "assemble failed"; exit 1; }
stamp_seed "$T/g.tape" "$SEED" "$T/g.exe" >/dev/null 2>&1
echo "interp tape: $(wc -c < "$T/g.tape" | tr -d ' ') B (compiled by bc)"

PASS=0; FAIL=0
ev() { # program  expected
  printf '%s' "$1" | "$T/g.exe"; got=$?
  if [ "$got" = "$2" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL got $got want $2 : $1"; fi
}
ov() { # program  expected-printed-value   (programs returning data structures)
  got=$(printf '%s' "$1" | "$T/g.exe")
  if [ "$got" = "$2" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL out '$got' want '$2' : $1"; fi
}
ev '(+ 2 3)' 5
ev '(- 50 8)' 42
ev '(let x 10 (* x x))' 100
ev '(if (lt 3 5) 42 0)' 42
ev '(if (eq 3 5) 1 0)' 0
ev '(def sq (x) (* x x)) (sq 9)' 81
ev '(def add (a b) (+ a b)) (add 10 20)' 30
ev '(def fac (n) (if (eq n 0) 1 (* n (fac (- n 1))))) (fac 5)' 120
ev '(def fib (n) (if (lt n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 10)' 55
ev '(def gcd (a b) (if (eq b 0) a (gcd b (% a b)))) (gcd 48 36)' 12
ev '(def sumto (n) (if (eq n 0) 0 (+ n (sumto (- n 1))))) (sumto 10)' 55
# Proper tail calls are required by generated state machines; this depth used to
# exhaust the Beta/Alpha return stack even though Gamma fuel remained available.
ev '(def loop (n) (if (eq n 0) 42 (loop (- n 1)))) (loop 10000)' 42
# stage 2 — algebraic data types + pattern matching
ev '(def toint (n) (match n (Z 0) ((S m) (+ 1 (toint m))))) (toint (S (S (S Z))))' 3
ev '(def len (xs) (match xs (Nil 0) ((Cons h t) (+ 1 (len t))))) (len (Cons 7 (Cons 8 (Cons 9 Nil))))' 3
ev '(def sum (xs) (match xs (Nil 0) ((Cons h t) (+ h (sum t))))) (sum (Cons 10 (Cons 20 (Cons 12 Nil))))' 42
ev '(def plus (a b) (match a (Z b) ((S m) (S (plus m b))))) (def toint (n) (match n (Z 0) ((S m) (+ 1 (toint m))))) (toint (plus (S (S Z)) (S (S Z))))' 4
ev '(def fst (p) (match p ((Pair a b) a))) (fst (Pair 42 99))' 42
ev '(def isnil (xs) (match xs (Nil 1) (other 0))) (isnil (Cons 1 Nil))' 0
ev '(def isnil (xs) (match xs (Nil 1) (other 0))) (isnil Nil)' 1
# returning data structures (printed)
ov '(def sq (xs) (match xs (Nil Nil) ((Cons h t) (Cons (* h h) (sq t))))) (sq (Cons 1 (Cons 2 (Cons 3 Nil))))' '(Cons 1 (Cons 4 (Cons 9 Nil)))'
ov '(def app (xs ys) (match xs (Nil ys) ((Cons h t) (Cons h (app t ys))))) (app (Cons 1 (Cons 2 Nil)) (Cons 3 Nil))' '(Cons 1 (Cons 2 (Cons 3 Nil)))'
ov '(def rev (xs acc) (match xs (Nil acc) ((Cons h t) (rev t (Cons h acc))))) (rev (Cons 1 (Cons 2 (Cons 3 Nil))) Nil)' '(Cons 3 (Cons 2 (Cons 1 Nil)))'
ov '(Pair (S (S Z)) Nil)' '(Pair (S (S Z)) Nil)'
# Cons compaction is representation-only: other arities retain generic ADT behavior.
ov '(Cons 1)' '(Cons 1)'
ov '(Cons 1 2 3)' '(Cons 1 2 3)'
# Values outside the small-integer intern range retain the ordinary boxed path.
ov '70001' '70001'
echo "gamma interp: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
