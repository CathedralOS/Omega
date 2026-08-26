#!/usr/bin/env sh
# CONVERGENCE — the proof-producing language and proof kernel meet. A Delta program (certify-add, compiled by
# the delta-rust aarch64 backend) reads two numbers, computes their sum, and EMITS A
# PROOF CERTIFICATE that the sum is correct. The trust anchor (the proof kernel,
# itself produced by the alpha->beta->bc pipeline) then independently verifies that
# certificate. So a computation up at the systems-language rung is checked by the
# proof checker at the trust-anchor rung: certifying computation, the Omega idea in
# miniature. A WRONG computation would emit a certificate proof kernel REJECTS -- the anchor
# checks the computation, not the compiler.
#
# Skips cleanly off macOS arm64 or without the cargo/clang toolchain.
set -e
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
SAMPLES="$OMEGA_PATH_DELTA/samples"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "convergence SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "convergence SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# 1. build the proof kernel (trust anchor), exactly as the lattice does
. "${OMEGA_PATH_BETA}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
stamp_beta_compiler "$T/bc.exe" >/dev/null \
  || { echo "convergence FAIL — Beta compiler artifact"; exit 1; }
if "$T/bc.exe" < "${OMEGA_PATH_PROOF_KERNEL}"/implementations/beta/check.beta > "$T/c.asm" 2>/dev/null \
   && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
   && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1; then :; else
  echo "convergence FAIL — could not build the proof kernel"; exit 1; fi

# 2. build certify-add (the certifying Delta program), via the aarch64 backend
cargo build -q 2>/dev/null || { echo "convergence FAIL — cargo build"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-add.alp" "$T/ca" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-add"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-lt.alp" "$T/clt" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-lt"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-bounds.alp" "$T/cb" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-bounds"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-divides.alp" "$T/cd" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-divides"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-accesses.alp" "$T/cacc" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-accesses"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-safety.alp" "$T/csaf" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-safety"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-source.alp" "$T/csrc" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-source"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-linked.alp" "$T/cl" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-linked"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-loop.alp" "$T/clp" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-loop"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-mul.alp" "$T/cm" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-mul"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-max.alp" "$T/cmax" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-max"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-sort2.alp" "$T/cs2" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-sort2"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-sort3.alp" "$T/cs3" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-sort3"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-is-sorted.alp" "$T/cis" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-is-sorted"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-member-any.alp" "$T/cma" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-member-any"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-max-any.alp" "$T/cxa" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-max-any"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-gcd.alp" "$T/cg" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-gcd"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-triangle.alp" "$T/ct" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-triangle"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-mod.alp" "$T/cmod" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-mod"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-member.alp" "$T/cmem" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-member"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-product.alp" "$T/cprod" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-product"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-factor.alp" "$T/cfac" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-factor"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-wrong.alp" "$T/cwrong" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-wrong"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-distinct.alp" "$T/cdist" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-distinct"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-sum.alp" "$T/csum" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-sum"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-palindrome.alp" "$T/cpal" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-palindrome"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-op.alp" "$T/cop" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-op"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-shape.alp" "$T/csh" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-shape"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-shapes.alp" "$T/css" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-shapes"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/certify-perm.alp" "$T/cperm" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-perm"; exit 1; }
# proof library: bounds-2d as a referenceable def, regenerated from the banked theorem
HAVE_LIB=0
if command -v python3 >/dev/null 2>&1 && python3 "${OMEGA_PATH_PROOF_KERNEL}"/tools/gen-lib2d.py > "$T/lib2d.proof" 2>/dev/null; then HAVE_LIB=1; fi

# the checker prints accept/reject to stdout but exits non-zero (the alpha VM's halt
# code), so judge by the stdout string, not the exit status -- and drop `set -e`.
set +e
PASS=0; FAIL=0
# the loop: delta emits a proof for a+b, the proof kernel confirms it
conv() {
  v=$(printf '%s %s' "$1" "$2" | "$T/ca" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1+$2 : proof kernel returned [$v], expected accept"; fi
}
conv 2 3; conv 7 5; conv 0 0; conv 12 8; conv 9 9

# the second program proves an ORDERING a < b with an existential witness (not refl)
clt() {
  v=$(printf '%s %s' "$1" "$2" | "$T/clt" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1<$2 : proof kernel returned [$v], expected accept"; fi
}
clt 2 5; clt 0 1; clt 7 12; clt 10 11; clt 3 100

# the third program proves its OWN 2D array-bounds VC:  i*n + j < m*n  (i n j m)
cb() {
  v=$(printf '%s %s %s %s' "$1" "$2" "$3" "$4" | "$T/cb" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [$1*$2+$3 < $4*$2] : proof kernel returned [$v], expected accept"; fi
}
cb 2 5 3 4; cb 0 8 0 1; cb 3 10 7 4; cb 1 1 0 2; cb 5 6 2 9

# the fourth program proves a MULTIPLICATIVE obligation: divisibility  n | a  (n a)
cd_() {
  v=$(printf '%s %s' "$1" "$2" | "$T/cd" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [$1 | $2] : proof kernel returned [$v], expected accept"; fi
}
cd_ 3 12; cd_ 5 20; cd_ 1 7; cd_ 7 7; cd_ 4 0; cd_ 6 42

# the division algorithm: a CONJUNCTION proof  a = q*m + r  AND  r < m  (inputs: a m)
cmod() {
  v=$(printf '%s %s' "$1" "$2" | "$T/cmod" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [$1 mod $2] : proof kernel returned [$v], expected accept"; fi
}
cmod 17 5; cmod 20 6; cmod 100 7; cmod 9 3; cmod 1 2; cmod 41 7

# a STRUCTURAL proof: list membership Mem(x, [a b c]) via a memtail/memhead walk (x a b c)
cmem() {
  v=$(printf '%s %s %s %s' "$1" "$2" "$3" "$4" | "$T/cmem" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [$1 in [$2 $3 $4]] : proof kernel returned [$v], expected accept"; fi
}
cmem 5 5 6 7; cmem 7 4 7 9; cmem 9 4 7 9; cmem 1 1 2 3; cmem 8 6 8 4

# the SECOND inductive predicate: ProdIs([a b c], a*b*c) -- the product-of-a-list FTA uses
cprod() {
  v=$(printf '%s %s %s' "$1" "$2" "$3" | "$T/cprod" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [prod($1,$2,$3)] : proof kernel returned [$v], expected accept"; fi
}
cprod 2 3 5; cprod 1 1 7; cprod 4 2 3; cprod 2 2 2; cprod 1 6 1

# COMPUTED factorization: read n, trial-divide into a factor list, certify ProdIs(factors, n).
# Where cprod asserts a given list, cfac computes it from n -- the FTA's existence half, executed.
cfac() {
  v=$(printf '%s' "$1" | "$T/cfac" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [factor($1)] : proof kernel returned [$v], expected accept"; fi
}
cfac 30; cfac 12; cfac 7; cfac 16; cfac 1; cfac 2; cfac 18

# THE NEGATIVE CONTROL: a deliberately-buggy computation (claims a+b = a+b+1) must be REJECTED.
# Without this, every accept-test above would pass vacuously if the checker degenerated to
# accept-everything -- this is the falsifiable form of "the anchor checks the computation".
cwrong() {
  v=$(printf '%s' "$1" | "$T/cwrong" | "$T/check.exe")
  if [ "$v" = reject ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [wrong($1)] : proof kernel returned [$v], expected REJECT"; fi
}
cwrong "2 3"; cwrong "7 5"; cwrong "0 0"; cwrong "10 4"

# a NEGATIVE fact (refutation): x != y, proved via sinj (injectivity) + disj (no-confusion)
cdist() {
  v=$(printf '%s %s' "$1" "$2" | "$T/cdist" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [$1 != $2] : proof kernel returned [$v], expected accept"; fi
}
cdist 3 5; cdist 5 3; cdist 7 0; cdist 0 4; cdist 1 2; cdist 100 99
# and the dual: a FALSE inequality (x != x) must be REJECTED -- the refutation must not lie
cdistno() {
  v=$(printf '%s %s' "$1" "$1" | "$T/cdist" | "$T/check.exe")
  if [ "$v" = reject ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  BREACH [$1 != $1 accepted] : proof kernel returned [$v], expected reject"; fi
}
cdistno 4; cdistno 0; cdistno 9

# computation over a USER-DEFINED function: the checker REDUCES a recursive `sum` over a
# user list to validate sum([a b c]) = a+b+c -- the fun/rec machinery, exercised here
csum() {
  v=$(printf '%s %s %s' "$1" "$2" "$3" | "$T/csum" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [sum($1,$2,$3)] : proof kernel returned [$v], expected accept"; fi
}
csum 2 3 5; csum 1 1 1; csum 4 0 6; csum 10 20 30; csum 0 0 0

# a verified PALINDROME: the checker reduces a recursive `reverse` -- which itself reduces a nested
# recursive `append` -- over a user list and confirms reverse(L) = L, a real recursive algorithm with a
# nested recursive call certifying a structural property of its input.
cpal() {
  v=$(printf '%s %s %s' "$1" "$2" "$3" | "$T/cpal" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [palindrome($1,$2,$3)] : proof kernel returned [$v], expected accept"; fi
}
cpal 2 3 2; cpal 1 1 1; cpal 5 0 5; cpal 7 7 7
# and a NON-palindrome must be REJECTED -- reverse(L) differs from L, so the certificate is false:
cpalno() {
  v=$(printf '%s %s %s' "$1" "$2" "$3" | "$T/cpal" | "$T/check.exe")
  if [ "$v" = reject ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  BREACH [non-palindrome($1,$2,$3) accepted] : proof kernel returned [$v], expected reject"; fi
}
cpalno 2 3 1; cpalno 5 0 9

# a SUM-TYPE-dispatched certifier: an `Op` enum (constructed from the op tag, then matched) picks
# sum vs product; the program proves the chosen result. Exercises enum construct/match + a
# value-returning self-call + a reassigned local -- the compiler's own machinery -- in the proof path.
cop() {
  v=$(printf '%s' "$1" | "$T/cop" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL op [$1] : proof kernel returned [$v], expected accept"; fi
}
cop "0 2 3"; cop "1 2 3"; cop "0 7 5"; cop "1 4 6"; cop "0 0 0"; cop "1 1 1"

# a PAYLOAD sum type: an enum whose variants CARRY data. Shape::Square(s) / Shape::Rect(w, h) --
# construction with variable payload fields, single AND multi-field binding (`{ s }`, `{ w, h }`),
# and binding-as-state-argument -- the full enum machinery, certifying the computed area.
csh() {
  v=$(printf '%s' "$1" | "$T/csh" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL shape [$1] : proof kernel returned [$v], expected accept"; fi
}
csh "0 4"; csh "0 5"; csh "0 9"; csh "1 6 7"; csh "1 3 9"; csh "1 2 8"
# tamper: shrink the first operand so (m S S) no longer equals the claimed area -- must reject.
badsh=$(printf '0 4' | "$T/csh" | sed 's/(s (s (s (s z))))/(s (s (s z)))/' | "$T/check.exe")
if [ "$badsh" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-shape : proof kernel returned [$badsh], expected reject"; fi

# the capstone: a counted list of shapes -> total area = nested sum of per-shape products. Composes a
# loop + per-element enum construct/match + payload binding + state args + mult + accumulation.
css() {
  v=$(printf '%s' "$1" | "$T/css" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL shapes [$1] : proof kernel returned [$v], expected accept"; fi
}
css "2  1 6 7  0 4"; css "3  0 2  0 3  1 4 5"; css "1  1 5 5"; css "0"; css "2  0 3  0 3"; css "4  0 1  0 2  0 3  0 4"
# tamper: shrink the first product's operand (6 -> 5) so the nested sum no longer equals the total.
badcss=$(printf '2  1 6 7  0 4' | "$T/css" | sed 's/(s (s (s (s (s (s z))))))/(s (s (s (s (s z)))))/' | "$T/check.exe")
if [ "$badcss" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-shapes : proof kernel returned [$badcss], expected reject"; fi

# the PERMUTATION predicate (Rel 779) -- the FTA inductive predicate, reached as a certifying computation:
# [a,b] ~ [b,a] by the adjacent-transposition rule. Exercises the new Perm checker rules through convergence.
cperm() {
  v=$(printf '%s' "$1" | "$T/cperm" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL perm [$1] : proof kernel returned [$v], expected accept"; fi
}
cperm "3 7"; cperm "5 2"; cperm "0 9"; cperm "11 4"
# tamper: shrink the first list's head so the goal's list no longer matches the permswap proof's argument.
badcp=$(printf '3 7' | "$T/cperm" | sed 's/(s (s (s z)))/(s (s z))/' | "$T/check.exe")
if [ "$badcp" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-perm : proof kernel returned [$badcp], expected reject"; fi

# CORRECTNESS (not safety): the result meets its spec -- m is genuinely max(a,b):
# a<=m & b<=m & (m=a or m=b). inl branch when a>=b, inr when a<b.
cmax() {
  v=$(printf '%s %s' "$1" "$2" | "$T/cmax" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL max($1,$2) : proof kernel returned [$v], expected accept"; fi
}
cmax 5 3; cmax 3 7; cmax 9 9; cmax 0 0; cmax 12 4

# a verified 2-element SORT: output is ordered AND a permutation of the input
cs2() {
  v=$(printf '%s %s' "$1" "$2" | "$T/cs2" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL sort2($1,$2) : proof kernel returned [$v], expected accept"; fi
}
cs2 5 3; cs2 3 7; cs2 4 4; cs2 0 9; cs2 100 2

# a verified 3-element SORT: certifies the output is ORDERED *and* a PERMUTATION of the input (the Perm
# inductive predicate), the permutation proved by a 3-compare-swap bubble network composed via permtrans.
# Functional correctness of a real sorting algorithm. Run across all orderings + ties.
cs3() {
  v=$(printf '%s %s %s' "$1" "$2" "$3" | "$T/cs3" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL sort3($1,$2,$3) : proof kernel returned [$v], expected accept"; fi
}
cs3 3 1 2; cs3 1 2 3; cs3 3 2 1; cs3 2 3 1; cs3 1 3 2; cs3 2 1 3; cs3 2 2 1; cs3 5 5 5

# a VARIABLE-LENGTH certifier: reads a list of ANY length and certifies it is SORTED via a right-nested
# conjunction of one adjacent-order witness per pair -- the proof size scales with the input, loop-driven.
cis() {
  v=$(printf '%s' "$1" | "$T/cis" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL is_sorted [$1] : proof kernel returned [$v], expected accept"; fi
}
cis "1 2"; cis "1 2 3"; cis "0 0 1 5"; cis "1 3 3 7 9"; cis "0 1 2 3 4 5"

# a variable-length INDUCTIVE-PREDICATE certifier: reads x + a list of any length and certifies Mem(x, list)
# by constructing a memtail-chain ending in a memhead -- the proof's length scales with x's position.
cma() {
  v=$(printf '%s' "$1" | "$T/cma" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL member [$1] : proof kernel returned [$v], expected accept"; fi
}
cma "3 3"; cma "2 1 2 3"; cma "5 9 4 5 1"; cma "0 7 3 0"; cma "6 2 4 6 8 10 12"

# a COMPUTED RESULT certified correct over a variable-length input: max(list) is THE max -- it bounds every
# element (a conjunction of order witnesses) AND occurs in the list (a Mem chain), composed in one proof.
cxa() {
  v=$(printf '%s' "$1" | "$T/cxa" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL max [$1] : proof kernel returned [$v], expected accept"; fi
}
cxa "5"; cxa "3 7"; cxa "9 4 5 1"; cxa "2 8 8 1"; cxa "10 2 6 10 4"

# a real ALGORITHM certified: Euclid's gcd output divides both inputs (g|a & g|b)
cg() {
  v=$(printf '%s %s' "$1" "$2" | "$T/cg" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL gcd($1,$2) : proof kernel returned [$v], expected accept"; fi
}
cg 12 8; cg 15 10; cg 7 3; cg 100 60; cg 0 9; cg 6 6

# a LOOP result certified against a CLOSED FORM: 2*(1+..+n) = n*(n+1) (Gauss)
ct() {
  v=$(printf '%s' "$1" | "$T/ct" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL triangle($1) : proof kernel returned [$v], expected accept"; fi
}
ct 4; ct 1; ct 0; ct 10; ct 20

# the certifying COMPILER: a whole program's worth of accesses, one conjunction proof
cacc() {
  v=$(printf '%s' "$1" | "$T/cacc" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL accesses [$1] : proof kernel returned [$v], expected accept"; fi
}
cacc "2 5 3 4"
cacc "2 5 3 4  1 3 0 2"
cacc "0 1 0 1  2 5 3 4  1 3 0 2  5 6 2 9"

# the certifying compiler over MIXED obligations: array-bounds (b) AND nonzero-divisor (d)
csaf() {
  v=$(printf '%s' "$1" | "$T/csaf" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL safety [$1] : proof kernel returned [$v], expected accept"; fi
}
csaf "d 5"
csaf "b 2 5 3 4  d 7  b 1 3 0 2"
csaf "b 0 1 0 1  d 2  b 5 6 2 9  d 100"

# the FRONTEND: compile tiny SOURCE (arr/get/div) to a safety proof checked by the proof kernel
csrc() {
  v=$(printf '%s' "$1" | "$T/csrc" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL source [$1] : proof kernel returned [$v], expected accept"; fi
}
csrc "arr 4 5  get 2 3"
csrc "arr 10 10  get 3 7  get 8 2  div 5"
csrc "arr 4 5  get 2 3  arr 2 6  get 1 4  div 3"
# bounded loops UNROLL to a range of VCs, all proved in the one whole-loop certificate
csrc "arr 4 5  band 4 0"
csrc "arr 10 10  band 8 3"
csrc "arr 4 5  band 2 1  div 7  get 3 4"
# the frontend's context binding must match hand-resolved obligations, byte for byte
if [ "$(printf 'arr 4 5  get 2 3  div 7  get 1 0' | "$T/csrc")" = "$(printf 'b 2 5 3 4  d 7  b 1 5 0 4' | "$T/csaf")" ]; then
  PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL source-vs-safety cross-check"; fi
# the frontend's OWN safety analysis: UNSAFE source is rejected (exit 1), not garbled
csrc_reject() {
  printf '%s' "$1" | "$T/csrc" >/dev/null 2>&1
  if [ $? -eq 1 ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL unsafe-not-rejected [$1]"; fi
}
csrc_reject "arr 4 5  get 3 7"        # 3*5+7=22 >= 20  (out of bounds)
csrc_reject "arr 4 5  get 2 3  div 0" # division by zero
csrc_reject "arr 5 5  get 5 0"        # 25 >= 25  (boundary, out of bounds)
csrc_reject "arr 4 5  band 5 0"       # loop overruns: iteration i=4 reaches row 4 of 0..3

# the compiler LINKS against a proof library: emit a proof that CITES bounds-2d (rather
# than re-derive the bound), prepend the library, and let the trust anchor check both.
if [ "$HAVE_LIB" = 1 ]; then
  linked() {
    v=$( { cat "$T/lib2d.proof"; printf ' '; printf '%s' "$1" | "$T/cl"; } | "$T/check.exe" )
    if [ "$v" = accept ]; then PASS=$((PASS+1)); else
      FAIL=$((FAIL+1)); echo "  FAIL linked [$1] : proof kernel returned [$v], expected accept"; fi
  }
  linked "2 5 3 4"; linked "0 8 0 1"; linked "5 6 2 9"
  # a tampered premise witness (wrong i<m) must reject even with a valid library
  bl=$( { cat "$T/lib2d.proof"; printf ' '; printf '2 5 3 4' | "$T/cl" | sed 's/(s z) (refl/(s (s z)) (refl/'; } | "$T/check.exe" )
  if [ "$bl" = reject ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL tamper-linked : proof kernel returned [$bl], expected reject"; fi

  # a WHOLE LOOP proved by ONE citation: forall i<K. i*n+j < m*n (composes lt-le-trans
  # + bounds-2d), instead of unrolling K per-iteration proofs.
  loop_link() {
    v=$( { cat "$T/lib2d.proof"; printf ' '; printf '%s' "$1" | "$T/clp"; } | "$T/check.exe" )
    if [ "$v" = accept ]; then PASS=$((PASS+1)); else
      FAIL=$((FAIL+1)); echo "  FAIL loop [$1] : proof kernel returned [$v], expected accept"; fi
  }
  loop_link "4 5 0 4"; loop_link "3 10 2 8"; loop_link "8 8 0 8"
  # citing the wrong library def (use 30 -> use 0) must be caught by the trust anchor
  bp=$( { cat "$T/lib2d.proof"; printf ' '; printf '4 5 0 4' | "$T/clp" | sed 's/(use 30)/(use 0)/'; } | "$T/check.exe" )
  if [ "$bp" = reject ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL tamper-loop : proof kernel returned [$bp], expected reject"; fi
  # overflow safety by citing mult-overflow (use 66): a*b < B*C from a<B and b<C
  mul_link() {
    v=$( { cat "$T/lib2d.proof"; printf ' '; printf '%s' "$1" | "$T/cm"; } | "$T/check.exe" )
    if [ "$v" = accept ]; then PASS=$((PASS+1)); else
      FAIL=$((FAIL+1)); echo "  FAIL mul [$1] : proof kernel returned [$v], expected accept"; fi
  }
  mul_link "3 4 5 6"; mul_link "7 2 8 9"; mul_link "0 0 1 1"
else echo "  (skipped linked-library checks — no python3 / library gen failed)"; fi

# CORRUPTED certificates must be rejected (the proof kernel checks the computation, not us):
# (a) claim 2+3 = 4; (b) reuse 2<5's witness to claim 2<4. Both must reject.
bad=$(printf '2 3' | "$T/ca" | sed 's/(s (s (s (s (s z)))))/(s (s (s (s z))))/g' | "$T/check.exe")
if [ "$bad" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-add : proof kernel returned [$bad], expected reject"; fi
badlt=$(printf '2 5' | "$T/clt" | sed 's/(s (s (s (s (s z)))))/(s (s (s (s z))))/g' | "$T/check.exe")
if [ "$badlt" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-lt : proof kernel returned [$badlt], expected reject"; fi
# (c) shrink the bounds witness by one so (i*n+j)+(w+1) no longer equals m*n.
badcb=$(printf '2 5 3 4' | "$T/cb" | sed 's/(s (s (s (s (s (s z))))))/(s (s (s (s (s z)))))/' | "$T/check.exe")
if [ "$badcb" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-bounds : proof kernel returned [$badcb], expected reject"; fi
# (d) shrink the cofactor so q*n no longer equals a (claim cofactor 3 for 3|12).
badcd=$(printf '3 12' | "$T/cd" | sed 's/(s (s (s (s z))))/(s (s (s z)))/' | "$T/check.exe")
if [ "$badcd" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-divides : proof kernel returned [$badcd], expected reject"; fi
# (e) corrupt ONE conjunct of a multi-access proof -- the whole conjunction must reject.
badca=$(printf '2 5 3 4  1 3 0 2' | "$T/cacc" | sed 's/(refl (m (s (s z)) (s (s (s z)))))/(refl (m (s (s z)) (s (s z))))/' | "$T/check.exe")
if [ "$badca" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-accesses : proof kernel returned [$badca], expected reject"; fi
# (f) corrupt the nonzero-divisor witness in a mixed proof -- must reject.
badcs=$(printf 'd 7' | "$T/csaf" | sed 's/(s (s (s (s (s (s z))))))/(s (s (s (s (s z)))))/' | "$T/check.exe")
if [ "$badcs" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-safety : proof kernel returned [$badcs], expected reject"; fi
# (g) the enum-dispatched certifier: claim 2*3 = 5 (shrink the product) -- must reject.
badop=$(printf '1 2 3' | "$T/cop" | sed 's/(s (s (s (s (s (s z))))))/(s (s (s (s (s z)))))/g' | "$T/check.exe")
if [ "$badop" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-op : proof kernel returned [$badop], expected reject"; fi

echo "convergence (delta emits a proof; the trust anchor checks it): $PASS confirmed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
