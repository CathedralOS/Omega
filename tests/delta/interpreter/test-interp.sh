#!/usr/bin/env sh
# Gate for the bounded Delta evaluation oracle. Compiles interp.gamma with the
# canonical Beta-written Gamma compiler, then evaluates candidate Delta
# programs and checks the integer result (the process exit code).
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
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_REPO_ROOT}"/tools/bootstrap/alpha/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
T=$(mktemp -d); trap 'rm -rf -- "$T"' EXIT
stamp_gamma_compiler "$T/gc.exe" >/dev/null

"$T/gc.exe" < interp.gamma > "$T/g.tape" || { echo "gc(interp.gamma) failed"; exit 1; }
stamp_seed "$T/g.tape" "$SEED" "$T/g.exe" >/dev/null 2>&1
echo "interp tape: $(wc -c < "$T/g.tape" | tr -d ' ') B (compiled by gc)"

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
reject_source() { # name source-file
  name=$1
  source_file=$2
  set +e
  "$T/g.exe" < "$source_file" > "$T/$name.out"
  got=$?
  if [ "$got" = 255 ] && [ ! -s "$T/$name.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL $name: source envelope returned $got with $(wc -c < "$T/$name.out" | tr -d ' ') output bytes"
  fi
}
trap_program() { # name program
  trap_name=$1
  trap_source=$2
  set +e
  { printf '%s' "$trap_source" | "$T/g.exe" > "$T/$trap_name.out"; } 2>/dev/null
  trap_status=$?
  if [ "$trap_status" = 132 ] && [ ! -s "$T/$trap_name.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL $trap_name: expected trap 132 with no output, got $trap_status with $(wc -c < "$T/$trap_name.out" | tr -d ' ') output bytes"
  fi
}
ev '(+ 2 3)' 5
ev '(- 50 8)' 42
ev '(let x 10 (* x x))' 100
ev '(if (lt 3 5) 42 0)' 42
ev '(if (eq 3 5) 1 0)' 0
ev '(def sq (x) (* x x)) (sq 9)' 81
ev '(def add (a b) (+ a b)) (add 10 20)' 30
cr_comment_program=$(printf '; before\r(+ 40 2)')
ev "$cr_comment_program" 42
unset cr_comment_program
printf '; hidden\000\n(+ 40 2)' > "$T/comment-nul.delta"
reject_source comment-nul "$T/comment-nul.delta"
printf '(+ 40\0132)' > "$T/vertical-tab.delta"
reject_source vertical-tab "$T/vertical-tab.delta"
printf '; hidden\177\n(+ 40 2)' > "$T/comment-del.delta"
reject_source comment-del "$T/comment-del.delta"
printf '; hidden\303\251\n(+ 40 2)' > "$T/comment-high.delta"
reject_source comment-high "$T/comment-high.delta"
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
# exhaust the Gamma/Alpha return stack even though Delta fuel remained available.
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
# Pattern-only constructor tags retain exact names and arities, including the
# arbitrary-constructor fallback.
ev '(match Nilly (Nil 1) (other 42))' 42
ev '(match (Consx 1 2) ((Cons a b) 1) (other 42))' 42
ev '(match (Cons 1 Nil) ((Cons a) 1) (other 42))' 42
ev '(match (Pair 1 2) ((Pair a) 1) (other 42))' 42
# Nonexhaustive source is statically invalid under D16. Until the direct
# compiler owns that rejection, both interpreter paths must fail loudly rather
# than turn the impossible state into integer zero.
trap_program no-match-tail '(match (Cons 1 Nil) (Nil 0))'
trap_program no-match-nested '(+ 1 (match (Cons 1 Nil) (Nil 0)))'
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
# Nonnegative u32 values use the oracle's immediate representation;
# zero/one, the adjacent value, negative arithmetic, and arithmetic crossing the
# boundary pin the evaluator's direct encode/decode paths and boxed fallback.
ov '0' '0'
ov '1' '1'
ov '4294967295' '4294967295'
ov '4294967296' '4294967296'
ov '(- 0 1)' '-1'
ov '(- 0 2)' '-2'
ov '(+ 4294967295 1)' '4294967296'
echo "delta interp: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
