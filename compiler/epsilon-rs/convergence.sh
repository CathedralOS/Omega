#!/usr/bin/env sh
# CONVERGENCE — the two top rungs meet. An epsilon program (certify-add, compiled by
# the epsilon-rs aarch64 backend) reads two numbers, computes their sum, and EMITS A
# DELTA CERTIFICATE that the sum is correct. The trust anchor (the delta checker,
# itself produced by the alpha->beta->bc pipeline) then independently verifies that
# certificate. So a computation up at the systems-language rung is checked by the
# proof checker at the trust-anchor rung: certifying computation, the Omega idea in
# miniature. A WRONG computation would emit a certificate delta REJECTS -- the anchor
# checks the computation, not the compiler.
#
# Skips cleanly off macOS arm64 or without the cargo/clang toolchain.
set -e
cd "$(dirname "$0")"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "convergence SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "convergence SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# 1. build the delta checker (trust anchor), exactly as the lattice does
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "convergence FAIL — bc build"; exit 1; }
if ../beta-lang-rs/build/bc.exe < ../delta/check.beta > "$T/c.asm" 2>/dev/null \
   && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
   && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1; then :; else
  echo "convergence FAIL — could not build the delta checker"; exit 1; fi

# 2. build certify-add (the certifying epsilon program), via the aarch64 backend
cargo build -q 2>/dev/null || { echo "convergence FAIL — cargo build"; exit 1; }
EPS_ARCH=aarch64 ./target/debug/beta samples/certify-add.alp "$T/ca" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-add"; exit 1; }
EPS_ARCH=aarch64 ./target/debug/beta samples/certify-lt.alp "$T/clt" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-lt"; exit 1; }
EPS_ARCH=aarch64 ./target/debug/beta samples/certify-bounds.alp "$T/cb" >/dev/null 2>&1 \
  || { echo "convergence FAIL — compiling certify-bounds"; exit 1; }

# the checker prints accept/reject to stdout but exits non-zero (the alpha VM's halt
# code), so judge by the stdout string, not the exit status -- and drop `set -e`.
set +e
PASS=0; FAIL=0
# the loop: epsilon emits a proof for a+b, the delta checker confirms it
conv() {
  v=$(printf '%s %s' "$1" "$2" | "$T/ca" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1+$2 : delta returned [$v], expected accept"; fi
}
conv 2 3; conv 7 5; conv 0 0; conv 12 8; conv 9 9

# the second program proves an ORDERING a < b with an existential witness (not refl)
clt() {
  v=$(printf '%s %s' "$1" "$2" | "$T/clt" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1<$2 : delta returned [$v], expected accept"; fi
}
clt 2 5; clt 0 1; clt 7 12; clt 10 11; clt 3 100

# the third program proves its OWN 2D array-bounds VC:  i*n + j < m*n  (i n j m)
cb() {
  v=$(printf '%s %s %s %s' "$1" "$2" "$3" "$4" | "$T/cb" | "$T/check.exe")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL [$1*$2+$3 < $4*$2] : delta returned [$v], expected accept"; fi
}
cb 2 5 3 4; cb 0 8 0 1; cb 3 10 7 4; cb 1 1 0 2; cb 5 6 2 9

# CORRUPTED certificates must be rejected (delta checks the computation, not us):
# (a) claim 2+3 = 4; (b) reuse 2<5's witness to claim 2<4. Both must reject.
bad=$(printf '2 3' | "$T/ca" | sed 's/(s (s (s (s (s z)))))/(s (s (s (s z))))/g' | "$T/check.exe")
if [ "$bad" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-add : delta returned [$bad], expected reject"; fi
badlt=$(printf '2 5' | "$T/clt" | sed 's/(s (s (s (s (s z)))))/(s (s (s (s z))))/g' | "$T/check.exe")
if [ "$badlt" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-lt : delta returned [$badlt], expected reject"; fi
# (c) shrink the bounds witness by one so (i*n+j)+(w+1) no longer equals m*n.
badcb=$(printf '2 5 3 4' | "$T/cb" | sed 's/(s (s (s (s (s (s z))))))/(s (s (s (s (s z)))))/' | "$T/check.exe")
if [ "$badcb" = reject ]; then PASS=$((PASS+1)); else
  FAIL=$((FAIL+1)); echo "  FAIL tamper-bounds : delta returned [$badcb], expected reject"; fi

echo "convergence (epsilon emits a delta proof; the trust anchor checks it): $PASS confirmed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
