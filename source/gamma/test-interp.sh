#!/usr/bin/env sh
# Gate for the Gamma reference interpreter (interp.beta, stage 1). Compiles it
# with bc (the self-hosting Rust-free Beta compiler), then evaluates Gamma
# programs and checks the integer result (the process exit code).
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
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
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
cv() { # constructor-valued program: canonical output and zero status
  got=$(printf '%s' "$1" | "$T/g.exe")
  status=$?
  if [ "$status" = 0 ] && [ "$got" = "$2" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL status $status out '$got' want '$2' : $1"; fi
}
ev '(+ 2 3)' 5
ev '(- 50 8)' 42
ev '(let x 10 (* x x))' 100
ev '(if (lt 3 5) 42 0)' 42
ev '(if (eq 3 5) 1 0)' 0
ev '(def sq (x) (* x x)) (sq 9)' 81
ev '(def add (a b) (+ a b)) (add 10 20)' 30
# Variable ASTs are resolved once to frame-local slots. Reusing the same body
# across calls, recursive re-entry, lets, shadowing, and match bindings must
# still observe the current invocation's values rather than cached values.
ev '(def choose (x) (let y (+ x 1) (match (Pair y x) ((Pair a b) (+ a b))))) (+ (choose 10) (choose 0))' 22
ev '(def shadow (x) (let y x (let x (+ y 1) (+ x y)))) (+ (shadow 10) (shadow 1))' 24
# A cached outer-frame variable read after a nested non-tail call must use the
# restored caller frame on every invocation, not the child frame or a value
# retained from the first invocation.
ev '(def child (x) (+ x 10)) (def outer (x) (+ (child (+ x 1)) x)) (+ (outer 3) (outer 7))' 42
ev '(def fac (n) (if (eq n 0) 1 (* n (fac (- n 1))))) (fac 5)' 120
ev '(def fib (n) (if (lt n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 10)' 55
ev '(def gcd (a b) (if (eq b 0) a (gcd b (% a b)))) (gcd 48 36)' 12
ev '(def sumto (n) (if (eq n 0) 0 (+ n (sumto (- n 1))))) (sumto 10)' 55
# Proper tail calls are required by generated state machines; this depth used to
# exhaust the Beta/Alpha return stack even though Gamma fuel remained available.
ev '(def loop (n) (if (eq n 0) 42 (loop (- n 1)))) (loop 10000)' 42
# Mutual tail transfers deliberately alternate frame arity. Cached variables in
# both bodies must follow the current transfer's frame while the trampoline
# continues to reuse its caller-owned frame base.
ev '(def narrow (n acc) (if (eq n 0) acc (wide (- n 1) (+ acc 1) 99))) (def wide (n acc unused) (if (eq n 0) acc (narrow (- n 1) (+ acc 1)))) (narrow 200 0)' 200
# stage 2 — algebraic data types + pattern matching
ev '(def toint (n) (match n (Z 0) ((S m) (+ 1 (toint m))))) (toint (S (S (S Z))))' 3
ev '(def len (xs) (match xs (Nil 0) ((Cons h t) (+ 1 (len t))))) (len (Cons 7 (Cons 8 (Cons 9 Nil))))' 3
ev '(def sum (xs) (match xs (Nil 0) ((Cons h t) (+ h (sum t))))) (sum (Cons 10 (Cons 20 (Cons 12 Nil))))' 42
ev '(def plus (a b) (match a (Z b) ((S m) (S (plus m b))))) (def toint (n) (match n (Z 0) ((S m) (+ 1 (toint m))))) (toint (plus (S (S Z)) (S (S Z))))' 4
ev '(def fst (p) (match p ((Pair a b) a))) (fst (Pair 42 99))' 42
ev '(def isnil (xs) (match xs (Nil 1) (other 0))) (isnil (Cons 1 Nil))' 0
ev '(def isnil (xs) (match xs (Nil 1) (other 0))) (isnil Nil)' 1
# Headerless persistent-array handles retain ordinary constructor matching.
ev '(match (Node 19 23) ((Node l r) (+ l r)))' 42
ev '(match (Chunks 2 ZeroTree) ((Chunks n t) n))' 2
# Pattern-only constructor tags retain exact names and arities, including the
# arbitrary-constructor fallback.
ev '(match Nilly (Nil 1) (other 42))' 42
ev '(match (Consx 1 2) ((Cons a b) 1) (other 42))' 42
ev '(match (Cons 1 Nil) ((Cons a) 1) (other 42))' 42
ev '(match (Pair 1 2) ((Pair a) 1) (other 42))' 42
# The translator's exact 524,288-slot tree carrier touches both boundary paths
# without materializing its zero subtrees.
ev '(def ntht (t k h) (if (eq h 0) t (match t ((Node l r) (if (lt k h) (ntht l k (/ h 2)) (ntht r (- k h) (/ h 2)))) (z 0)))) (def nth (xs k) (match xs ((Chunks n t) (ntht t k 262144)))) (def sett (t k v h) (if (eq h 0) v (match t ((Node l r) (if (lt k h) (Node (sett l k v (/ h 2)) r) (Node l (sett r (- k h) v (/ h 2))))) (z (if (lt k h) (Node (sett 0 k v (/ h 2)) 0) (Node 0 (sett 0 (- k h) v (/ h 2)))))))) (def setl (xs k v) (match xs ((Chunks n t) (Chunks n (sett t k v 262144))))) (let a (setl (Chunks 524288 0) 0 19) (let b (setl a 524287 23) (+ (nth b 0) (nth b 524287))))' 42
# returning data structures (printed)
cv '(def sq (xs) (match xs (Nil Nil) ((Cons h t) (Cons (* h h) (sq t))))) (sq (Cons 1 (Cons 2 (Cons 3 Nil))))' '(Cons 1 (Cons 4 (Cons 9 Nil)))'
cv '(def app (xs ys) (match xs (Nil ys) ((Cons h t) (Cons h (app t ys))))) (app (Cons 1 (Cons 2 Nil)) (Cons 3 Nil))' '(Cons 1 (Cons 2 (Cons 3 Nil)))'
cv '(def rev (xs acc) (match xs (Nil acc) ((Cons h t) (rev t (Cons h acc))))) (rev (Cons 1 (Cons 2 (Cons 3 Nil))) Nil)' '(Cons 3 (Cons 2 (Cons 1 Nil)))'
cv '(Pair (S (S Z)) Nil)' '(Pair (S (S Z)) Nil)'
# Cons compaction is representation-only: other arities retain generic ADT behavior.
cv '(Cons 1)' '(Cons 1)'
cv '(Cons 1 2 3)' '(Cons 1 2 3)'
# A formerly boxed compiler-sized value remains exact under the immediate path.
ov '70001' '70001'
# Nonnegative u32 values use the canonical interpreter's immediate representation;
# zero/one, the adjacent value, negative arithmetic, and arithmetic crossing the
# boundary pin the evaluator's direct encode/decode paths and boxed fallback.
ov '0' '0'
ov '1' '1'
ov '4294967295' '4294967295'
ov '4294967296' '4294967296'
ov '(- 0 1)' '-1'
ov '(- 0 2)' '-2'
ov '(+ 4294967295 1)' '4294967296'
# Known persistent-array constructors use compact tagged handles without changing
# their canonical printed tree.
cv '(Chunks 2 (Node ZeroTree ZeroTree))' '(Chunks 2 (Node ZeroTree ZeroTree))'
echo "gamma interp: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
