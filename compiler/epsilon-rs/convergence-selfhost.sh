#!/usr/bin/env sh
# SELF-HOSTED CONVERGENCE — the convergence loop with Rust removed from the certified path's compiler.
#
# convergence.sh shows: a certifying epsilon program (compiled by the Rust aarch64 backend) emits a
# DELTA CERTIFICATE that the alpha->beta->bc trust anchor independently checks. This script does the
# same, but the certifying program is compiled by the SELF-HOSTED epsilon compiler -- lowermachine.alp,
# itself an epsilon program that the backend lowers to arm64 and that reproduces its own source
# byte-for-byte (the FIXPOINT in test_aarch64.sh). So the entire upper lattice agrees: an epsilon-written
# compiler builds the computation, and an alpha-rooted proof checker validates its proof, with the Rust
# backend nowhere in the trusted-output path. (selfhost-sweep.sh already shows lowermachine == the Rust
# backend across the corpus, so the emitted certificate is identical; this wires that into delta directly.)
#
# Skips cleanly off macOS arm64 or without the toolchain.
set -e
cd "$(dirname "$0")"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "self-hosted convergence SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "self-hosted convergence SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# 1. the trust anchor: the delta checker, built via alpha -> beta -> bc (identical to convergence.sh)
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "self-hosted convergence FAIL — bc build"; exit 1; }
if ../beta-lang-rs/build/bc.exe < ../delta/check.beta > "$T/c.asm" 2>/dev/null \
   && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
   && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1; then :; else
  echo "self-hosted convergence FAIL — could not build the delta checker"; exit 1; fi

# 2. the SELF-HOSTED epsilon compiler: lowermachine, lowered by the backend, then used to compile certify-*.
#    lmx emits arm64 asm to stdout; clang assembles + signs it. (No EPS_ARCH backend on the certified path.)
cargo build -q 2>/dev/null || { echo "self-hosted convergence FAIL — cargo build"; exit 1; }
EPS_ARCH=aarch64 ./target/debug/beta samples/lowermachine.alp "$T/lmx" >/dev/null 2>&1 \
  || { echo "self-hosted convergence FAIL — building lowermachine"; exit 1; }
build_lm() {  # build_lm SAMPLE OUTNAME
  "$T/lmx" < "samples/$1.alp" > "$T/$2.s" 2>/dev/null \
    && clang -arch arm64 -o "$T/$2" "$T/$2.s" 2>/dev/null \
    && codesign -f -s - "$T/$2" 2>/dev/null \
    || { echo "self-hosted convergence FAIL — lowermachine compiling $1"; exit 1; }
}
build_lm certify-add      ca
build_lm certify-lt       clt
build_lm certify-bounds   cb
build_lm certify-divides  cd
build_lm certify-mod      cmod
build_lm certify-member   cmem
build_lm certify-product  cprod
build_lm certify-distinct cdist
build_lm certify-sum      csum
build_lm certify-max      cmax
build_lm certify-sort2    cs2
build_lm certify-gcd      cg
build_lm certify-triangle ct
build_lm certify-op       cop
build_lm certify-shape    csh

# the checker prints accept/reject but exits with the alpha VM's halt code, so judge by stdout.
set +e
PASS=0; FAIL=0
chk() {  # chk OUTNAME "INPUT" EXPECT
  v=$(printf '%s' "$2" | "$T/$1" | "$T/check.exe")
  if [ "$v" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 [$2] : delta returned [$v], expected $3"; fi
}
# certifying computations across the corpus, each compiled by the SELF-HOSTED compiler and checked
# by the trust anchor -- arithmetic, ordering, structure, bounds, divisibility, refutation, recursion:
chk ca    "2 3"     accept; chk ca    "7 5"     accept; chk ca "0 0" accept   # a + b
chk clt   "2 5"     accept; chk clt   "10 11"   accept                         # a < b (existential witness)
chk cb    "2 5 3 4" accept; chk cb    "1 1 0 2" accept                         # 2D array-bounds VC
chk cd    "3 12"    accept; chk cd    "7 7"     accept                         # divisibility n | a
chk cmod  "17 5"    accept; chk cmod  "9 3"     accept                         # a = q*m + r AND r < m
chk cmem  "5 5 6 7" accept; chk cmem  "9 4 7 9" accept                         # list membership (structural)
chk cprod "2 3 5"   accept; chk cprod "4 2 3"   accept                         # ProdIs([a b c], a*b*c)
chk cdist "3 5"     accept; chk cdist "0 4"     accept                         # x != y (refutation: sinj+disj)
chk csum  "2 3 5"   accept; chk csum  "10 20 30" accept                        # sum over a user list (rec reduction)
chk cmax  "5 3"     accept; chk cmax  "3 7"     accept                         # m = max(a,b), correctness spec
chk cs2   "5 3"     accept; chk cs2   "0 9"     accept                         # 2-element sort: ordered + permutation
chk cg    "12 8"    accept; chk cg    "100 60"  accept                         # Euclid gcd divides both
chk ct    "4"       accept; chk ct    "10"      accept                         # loop result = closed form (Gauss)
# a SUM-TYPE-dispatched certifier: an `Op` enum (construct + match) picks sum vs product, then proves
# the result -- exercising the self-hosted compiler's enum/value-call/local machinery in the proof path
chk cop   "0 2 3"   accept; chk cop   "1 2 3"   accept                         # Op::Sum a+b ; Op::Prod a*b
chk cop   "0 7 5"   accept; chk cop   "1 4 6"   accept
# a PAYLOAD sum type: Shape::Square(s) / Shape::Rect(w,h) -- construction with variable fields, single
# AND multi-field binding, state args -- the self-hosted compiler's full enum machinery in the proof path
chk csh   "0 4"     accept; chk csh   "1 6 7"   accept                         # Square s*s ; Rect w*h
chk csh   "0 5"     accept; chk csh   "1 3 9"   accept

# the dual must hold too: a FALSE refutation (x != x) must be REJECTED, even self-hosted-compiled
chk cdist "4 4"     reject; chk cdist "0 0"     reject

# the trust anchor still REJECTS tampered certificates from self-hosted-compiled programs -- delta
# checks the computation, not the compiler that built it:
bad=$(printf '2 3' | "$T/ca" | sed 's/(s (s (s (s (s z)))))/(s (s (s (s z))))/g' | "$T/check.exe")            # claim 2+3 = 4
if [ "$bad" = reject ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL tamper-add : [$bad], expected reject"; fi
badlt=$(printf '2 5' | "$T/clt" | sed 's/(s (s (s (s (s z)))))/(s (s (s (s z))))/g' | "$T/check.exe")         # shrink the < witness
if [ "$badlt" = reject ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL tamper-lt : [$badlt], expected reject"; fi
badcd=$(printf '3 12' | "$T/cd" | sed 's/(s (s (s (s z))))/(s (s (s z)))/' | "$T/check.exe")                  # shrink the cofactor
if [ "$badcd" = reject ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL tamper-divides : [$badcd], expected reject"; fi

echo "self-hosted convergence (the SELF-HOSTED epsilon compiler's certifiers, checked by the trust anchor): $PASS confirmed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
echo "SELF-HOSTED CONVERGENCE ✓ — lowermachine-compiled certifiers emit proofs the alpha-rooted trust anchor accepts"
