#!/usr/bin/env sh
# Gate for the Gamma static type checker (typeck.beta). Compiled by bc, run on the
# seed. Well-typed programs -> exit 1; type errors -> exit 0.
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
T=$(mktemp -d); trap 'trash "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
"$T/bc.exe" < typeck.beta > "$T/tc.tape" || { echo "bc(typeck.beta) failed"; exit 1; }
stamp_seed "$T/tc.tape" "$SEED" "$T/tc.exe" >/dev/null 2>&1

PASS=0; FAIL=0
tc() { # program  expect(1 ok / 0 type-error)  desc
  printf '%s' "$1" | "$T/tc.exe"; got=$?
  if [ "$got" = "$2" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL want $2 got $got : $3"; fi
}
reject_source() { # name source-file
  name=$1
  source_file=$2
  set +e
  "$T/tc.exe" < "$source_file" > "$T/$name.out"
  got=$?
  if [ "$got" = 0 ] && [ ! -s "$T/$name.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL $name: source envelope returned $got with $(wc -c < "$T/$name.out" | tr -d ' ') output bytes"
  fi
}
accept_source() { # name source-file
  name=$1
  source_file=$2
  set +e
  "$T/tc.exe" < "$source_file" > "$T/$name.out"
  got=$?
  if [ "$got" = 1 ] && [ ! -s "$T/$name.out" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1))
    echo "  FAIL $name: valid source returned $got with $(wc -c < "$T/$name.out" | tr -d ' ') output bytes"
  fi
}
# phase 1 — Int + typed functions
tc '(def add ((a Int) (b Int)) Int (+ a b)) (def main () Int (add 2 3))' 1 'well-typed'
cr_comment_program=$(printf '; before\r(def id ((x Int)) Int x)')
tc "$cr_comment_program" 1 'CR-terminated comment'
unset cr_comment_program
printf '; hidden\000\n(def id ((x Int)) Int x)' > "$T/comment-nul.gamma"
reject_source comment-nul "$T/comment-nul.gamma"
printf '(def\013id ((x Int)) Int x)' > "$T/vertical-tab.gamma"
reject_source vertical-tab "$T/vertical-tab.gamma"
printf '; hidden\177\n(def id ((x Int)) Int x)' > "$T/comment-del.gamma"
reject_source comment-del "$T/comment-del.gamma"
printf '; hidden\303\251\n(def id ((x Int)) Int x)' > "$T/comment-high.gamma"
reject_source comment-high "$T/comment-high.gamma"
# Place the second declaration exactly at source offset 2 MiB. The former
# declaration tables began at raw address 4 MiB and therefore aliased this byte
# because the source buffer begins at raw address 2 MiB.
printf '(def first () Int (later))\n;' > "$T/table-disjoint.gamma"
table_prefix_size=$(wc -c < "$T/table-disjoint.gamma" | tr -d ' ')
table_pad_size=$((2097151 - table_prefix_size))
dd if=/dev/zero bs="$table_pad_size" count=1 2>/dev/null | tr '\000' 'x' >> "$T/table-disjoint.gamma"
printf '\n(def later () Int 7)' >> "$T/table-disjoint.gamma"
accept_source table-disjoint "$T/table-disjoint.gamma"
unset table_prefix_size table_pad_size
# Cross the retired interpreter's 512-value scratch bound. Gamma arity is a
# language property, so frontend parsing/checking must not recurse once per row
# or inherit that oracle-private ceiling.
awk 'BEGIN {
  printf "(def wide ("
  for (i = 0; i < 600; i++) printf "(p%d Int)", i
  printf ") Int p599) (def main () Int (wide"
  for (i = 0; i < 600; i++) printf " %d", i
  print "))"
}' > "$T/wide-call.gamma"
accept_source wide-call "$T/wide-call.gamma"
awk 'BEGIN {
  printf "(data Wide (Mk"
  for (i = 0; i < 600; i++) printf " Int"
  printf ")) (def make () Wide (Mk"
  for (i = 0; i < 600; i++) printf " %d", i
  printf ")) (def last ((w Wide)) Int (match w ((Mk"
  for (i = 0; i < 600; i++) printf " x%d", i
  print ") x599)))"
}' > "$T/wide-constructor.gamma"
accept_source wide-constructor "$T/wide-constructor.gamma"
awk 'BEGIN {
  for (i = 0; i <= 32768; i++) printf "(def f%d () Int 0)\n", i
}' > "$T/function-capacity.gamma"
reject_source function-capacity "$T/function-capacity.gamma"
awk 'BEGIN {
  printf "(def f ((x Int)) Int x) (def main () Int (f"
  for (i = 0; i < 300000; i++) printf " 0"
  print "))"
}' > "$T/arena-capacity.gamma"
reject_source arena-capacity "$T/arena-capacity.gamma"
awk 'BEGIN {
  printf "(def main () Int "
  for (i = 0; i < 900; i++) printf "(let x%d 0 ", i
  printf "0"
  for (i = 0; i < 900; i++) printf ")"
  print ")"
}' > "$T/nesting-within-profile.gamma"
accept_source nesting-within-profile "$T/nesting-within-profile.gamma"
awk 'BEGIN {
  printf "(def main () Int "
  for (i = 0; i < 1100; i++) printf "(let x%d 0 ", i
  printf "0"
  for (i = 0; i < 1100; i++) printf ")"
  print ")"
}' > "$T/nesting-exhausted.gamma"
reject_source nesting-exhausted "$T/nesting-exhausted.gamma"
# fixed D16 program/declaration grammar and exact source exhaustion
tc '' 0 'empty program'
tc '; comment only' 0 'comment-only program'
tc '(data Nat (Z))' 0 'data-only program'
tc 'junk' 0 'junk-only program'
tc '(def main () Int 0) junk' 0 'trailing token'
tc '(def main () Int 0) (data Nat (Z))' 0 'data after function'
tc '(def main () Int 0))' 0 'stray closing delimiter'
tc '(def main () Int 0' 0 'missing closing delimiter'
tc '(fun main () Int 0)' 0 'unknown top-level declaration'
tc '(def main () Int (if 1 2 3 4))' 0 'if has exact arity'
tc '(def main () Int (+ 1 2 3))' 0 'operator has exact arity'
tc '(data Nat (Z)) (def main ((n Nat)) Int (match n (Z 0 1)))' 0 'match arm has exact arity'
tc '(data nat (Z)) (def main () Int 0)' 0 'declared type begins uppercase'
tc '(data Nat (z)) (def main () Int 0)' 0 'constructor begins uppercase'
tc '(def Main () Int 0)' 0 'function begins lowercase or underscore'
tc '(def main ((X Int)) Int X)' 0 'parameter begins lowercase or underscore'
tc '(def main () Int (let X 1 X))' 0 'local begins lowercase or underscore'
tc '(def if () Int 0)' 0 'keyword cannot be a declaration name'
tc '(def bytes_empty () Int 0)' 0 'Bytes builtin cannot be a declaration name'
tc '(def main () Int (let match 1 match))' 0 'keyword cannot be a binder'
tc '(data Bytes (B)) (def main () Int 0)' 0 'builtin type cannot be redeclared'
tc '(data Nat) (def main () Int 0)' 0 'data requires a constructor'
tc '(def main () Bytes bytes_empty)' 0 'Bytes builtin is not a bare variable'
tc '(data A garbage) (def main () Int 0)' 0 'invalid constructor punctuation rejects without nonprogress'
tc '(def id ((x Int)) Int x)' 1 'identity'
tc '(def f ((a Int) (b Int)) Int (if (lt a b) a b))' 1 'if/branches'
tc '(def f ((a Int)) Int (let y (+ a 1) (* y y)))' 1 'let'
tc '(def f ((a Int)) Int (g a)) (def g ((x Int)) Int x)' 1 'forward call'
tc '(def min () Int -9223372036854775808) (def max () Int 9223372036854775807)' 1 'signed Int literal bounds'
tc '(def bad () Int 9223372036854775808)' 0 'positive Int literal overflow'
tc '(def bad () Int -9223372036854775809)' 0 'negative Int literal overflow'
tc '(def add ((a Int) (b Int)) Int (+ a b)) (def main () Int (add 2))' 0 'arity too few'
tc '(def add ((a Int) (b Int)) Int (+ a b)) (def main () Int (add 1 2 3))' 0 'arity too many'
tc '(def main () Int (nope 1))' 0 'unknown function'
tc '(def f ((x Nope)) Nope x)' 0 'unknown declared type'
# D16 compact Bytes type and closed builtin signatures
tc '(def one () Bytes (bytes_single 255)) (def main ((b Bytes)) Bytes (bytes_concat (bytes_empty) (bytes_slice b 0 (bytes_length b))))' 1 'Bytes constructors and views'
tc '(def main ((b Bytes)) Int (bytes_get b 0))' 1 'Bytes indexed read'
tc '(def bad () Bytes (bytes_single (bytes_empty)))' 0 'bytes_single requires Int'
tc '(def bad ((b Bytes)) Int (bytes_length 1))' 0 'bytes_length requires Bytes'
tc '(def bad ((b Bytes)) Int (bytes_get b))' 0 'bytes_get arity'
tc '(def bad ((b Bytes)) Bytes (bytes_slice b 0))' 0 'bytes_slice arity'
tc '(def bad ((b Bytes)) Bytes (bytes_concat b 0))' 0 'bytes_concat argument type'
tc '(def bad ((b Bytes)) Int (match b (rest 0)))' 0 'Bytes is not algebraic'
# phase 2 — data declarations (ADTs) + match, well-typed
tc '(data Nat (Z) (S Nat)) (def pred ((n Nat)) Nat (match n (Z Z) ((S m) m))) (def main () Nat (pred (S (S Z))))' 1 'Nat pred'
tc '(data List (Nil) (Cons Int List)) (def len ((xs List)) Int (match xs (Nil 0) ((Cons h t) (+ 1 (len t)))))' 1 'list length'
tc '(data Nat (Z) (S Nat)) (def plus ((a Nat) (b Nat)) Nat (match a (Z b) ((S m) (S (plus m b)))))' 1 'Nat plus'
tc '(data A (MkA B)) (data B (MkB A)) (def keep ((a A)) A a)' 1 'forward and mutual nominal type references'
tc '(data Nat (Z) (S Nat)) (def classify ((n Nat)) Int (match n (Z 0) (rest 1)))' 1 'final catch-all is exhaustive'
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
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n (Z 0)))' 0 'missing constructor arm'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n (Z 0) (Z 1) ((S m) 2)))' 0 'duplicate constructor arm'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n (rest 0) (Z 1)))' 0 'arm after catch-all'
tc '(def bad ((n Int)) Int (match n (rest 0)))' 0 'match requires algebraic scrutinee'
tc '(data Nat (Z) (S Nat)) (def bad ((n Nat)) Int (match n))' 0 'match requires an arm'
echo "gamma typeck: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
