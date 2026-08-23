#!/usr/bin/env sh
# SELF-HOSTED CONVERGENCE — the convergence loop with Rust removed from the certified path's compiler.
#
# convergence.sh shows: a certifying Delta program (compiled by the Rust aarch64 backend) emits a
# PROOF CERTIFICATE that the alpha->beta->bc trust anchor independently checks. This script does the
# same, but the certifying program is compiled by the SELF-HOSTED Delta compiler -- lowermachine.alp,
# itself a Delta program that the backend lowers to arm64 and that reproduces its own source
# byte-for-byte (the FIXPOINT in test_aarch64.sh). So the entire upper lattice agrees: a Delta-written
# compiler builds the computation, and an alpha-rooted proof checker validates its proof, with the Rust
# backend nowhere in the trusted-output path. (selfhost-sweep.sh already shows lowermachine == the Rust
# backend across the corpus, so the emitted certificate is identical; this wires that into delta directly.)
#
# Skips cleanly off macOS arm64 or without the toolchain.
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
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "self-hosted convergence SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "self-hosted convergence SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# 1. the trust anchor: the proof kernel, built via alpha -> beta -> bc (identical to convergence.sh)
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
( cd "${OMEGA_PATH_BETA_COMPILER_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null ) || { echo "self-hosted convergence FAIL — bc build"; exit 1; }
if "${OMEGA_PATH_BETA_COMPILER_RUST}"/build/bc.exe < "${OMEGA_PATH_PROOF_KERNEL}"/implementations/beta/check.beta > "$T/c.asm" 2>/dev/null \
   && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
   && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1; then :; else
  echo "self-hosted convergence FAIL — could not build the proof kernel"; exit 1; fi

# 1b. the PROOF LIBRARY (bounds-2d=30, lt-le-trans=34, mult-overflow=66) for the library-citing certifiers.
HAVE_LIB=0
if command -v python3 >/dev/null 2>&1 && python3 "${OMEGA_PATH_PROOF_KERNEL}"/tools/gen-lib2d.py > "$T/lib2d.proof" 2>/dev/null; then HAVE_LIB=1; fi

# 2. the SELF-HOSTED Delta compiler: lowermachine, lowered by the backend, then used to compile certify-*.
#    lmx emits arm64 asm to stdout; clang assembles + signs it. (No DELTA_ARCH backend on the certified path.)
cargo build -q 2>/dev/null || { echo "self-hosted convergence FAIL — cargo build"; exit 1; }
DELTA_ARCH=aarch64 ./target/debug/delta "$SAMPLES/lowermachine.alp" "$T/lmx" >/dev/null 2>&1 \
  || { echo "self-hosted convergence FAIL — building lowermachine"; exit 1; }
build_lm() {  # build_lm SAMPLE OUTNAME
  "$T/lmx" < "$SAMPLES/$1.alp" > "$T/$2.s" 2>/dev/null \
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
build_lm certify-factor   cfac
build_lm certify-wrong    cwrong
build_lm certify-distinct cdist
build_lm certify-sum      csum
build_lm certify-max      cmax
build_lm certify-sort2    cs2
build_lm certify-gcd      cg
build_lm certify-triangle ct
build_lm certify-op       cop
build_lm certify-shape    csh
build_lm certify-shapes   css
build_lm certify-perm     cperm
build_lm certify-palindrome cpal
build_lm certify-sort3    cs3
build_lm certify-is-sorted cis
build_lm certify-member-any cma
build_lm certify-max-any   cxa
build_lm certify-accesses  cacc
build_lm certify-safety    csaf
build_lm certify-source    csrc
build_lm certify-linked    cl
build_lm certify-loop      clp
build_lm certify-mul       cm

# the checker prints accept/reject but exits with the alpha VM's halt code, so judge by stdout.
set +e
PASS=0; FAIL=0
chk() {  # chk OUTNAME "INPUT" EXPECT
  v=$(printf '%s' "$2" | "$T/$1" | "$T/check.exe")
  if [ "$v" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 [$2] : proof kernel returned [$v], expected $3"; fi
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
chk cfac  "30"      accept; chk cfac  "12"      accept                         # COMPUTED factorization: ProdIs(factors, n)
chk cwrong "2 3"    reject; chk cwrong "7 5"    reject                         # NEGATIVE CONTROL: buggy a+b+1 claim REJECTED
chk cdist "3 5"     accept; chk cdist "0 4"     accept                         # x != y (refutation: sinj+disj)
chk csum  "2 3 5"   accept; chk csum  "10 20 30" accept                        # sum over a user list (rec reduction)
chk cmax  "5 3"     accept; chk cmax  "3 7"     accept                         # m = max(a,b), correctness spec
chk cs2   "5 3"     accept; chk cs2   "0 9"     accept                         # 2-element sort: ordered + permutation
chk cs3   "3 1 2"   accept; chk cs3   "3 2 1"   accept; chk cs3 "1 2 3" accept # 3-element sort: ordered + Perm
chk cpal  "2 3 2"   accept; chk cpal  "5 0 5"   accept; chk cpal "2 3 1" reject # palindrome: reverse(L)=L
chk cis   "1 2 3"   accept; chk cis   "0 0 1 5" accept; chk cis "1 3 3 7 9" accept # variable-length: list is sorted
chk cma   "2 1 2 3" accept; chk cma   "5 9 4 5 1" accept; chk cma "6 2 4 6 8 10 12" accept # variable-length Mem proof
chk cxa   "3 7"     accept; chk cxa   "9 4 5 1" accept; chk cxa "10 2 6 10 4" accept # computed max correct (bound + member)
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
# the CAPSTONE: a counted list of shapes, total area = nested sum of per-shape products -- composes a
# loop + per-element enum construct/match + binding + state args + mult + accumulation, all certified
chk css   "2  1 6 7  0 4"     accept; chk css   "3  0 2  0 3  1 4 5"  accept    # 42+16=58 ; 4+9+20=33
chk css   "1  1 5 5"          accept; chk css   "0"                  accept    # single Rect ; empty list (z=0)
# the PERMUTATION predicate (Rel 779, the FTA inductive predicate) reached through the self-hosted compiler:
# [a,b] is a permutation of [b,a] by the adjacent-transposition rule (permswap)
chk cperm "3 7"     accept; chk cperm "5 2"     accept; chk cperm "0 9" accept

# the dual must hold too: a FALSE refutation (x != x) must be REJECTED, even self-hosted-compiled
chk cdist "4 4"     reject; chk cdist "0 0"     reject

# the certifying COMPILER + FRONTEND, self-hosted: a whole program's array accesses discharged as ONE
# conjunction proof (cacc), mixed bounds+nonzero-divisor obligations (csaf), and a source-DSL frontend
# that compiles arr/get/div programs to a delta-checked whole-program safety proof (csrc).
chk cacc "2 5 3 4"           accept; chk cacc "0 1 0 1  2 5 3 4  1 3 0 2  5 6 2 9" accept
chk csaf "d 5"               accept; chk csaf "b 2 5 3 4  d 7  b 1 3 0 2"          accept
chk csrc "arr 4 5  get 2 3"  accept; chk csrc "arr 10 10  get 3 7  get 8 2  div 5" accept
chk csrc "arr 4 5  band 4 0" accept   # a bounded loop unrolls to a range of VCs in one certificate
# the self-hosted FRONTEND's OWN safety analysis: UNSAFE source (out-of-bounds / div-by-zero / loop
# overrun) is refused -- it exits 1 and emits NO proof, rather than emitting one the anchor would reject.
csrc_reject() { printf '%s' "$1" | "$T/csrc" >/dev/null 2>&1; if [ $? -eq 1 ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL unsafe-not-rejected [$1]"; fi; }
csrc_reject "arr 4 5  get 3 7"; csrc_reject "arr 4 5  get 2 3  div 0"; csrc_reject "arr 5 5  get 5 0"; csrc_reject "arr 4 5  band 5 0"

# the self-hosted compiler LINKING against a proof library (lib2d.proof from gen-lib2d.py): emit only the
# LINKAGE that CITES a banked theorem with site witnesses, prepend the library, and let the trust anchor
# check both. This completes the keystone -- the ENTIRE certify corpus self-hosted, Rust off the path.
if [ "$HAVE_LIB" = 1 ]; then
  lchk() { v=$( { cat "$T/lib2d.proof"; printf ' '; printf '%s' "$2" | "$T/$1"; } | "$T/check.exe" ); if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $1 [$2] : delta [$v], expected accept"; fi; }
  lchk cl  "2 5 3 4"; lchk cl  "0 8 0 1"; lchk cl  "5 6 2 9"   # bounds-2d (use 30) cited with i<m, j<n witnesses
  lchk clp "4 5 0 4"; lchk clp "3 10 2 8"; lchk clp "8 8 0 8"  # whole loop forall i<K: lt-le-trans (34) into bounds-2d
  lchk cm  "3 4 5 6"; lchk cm  "7 2 8 9"; lchk cm  "0 0 1 1"   # mult-overflow (use 66): a*b<B*C from a<B, b<C
  # a tampered premise / wrong-def citation must REJECT even with a valid library present
  blk=$( { cat "$T/lib2d.proof"; printf ' '; printf '2 5 3 4' | "$T/cl" | sed 's/(s z) (refl/(s (s z)) (refl/'; } | "$T/check.exe" )
  if [ "$blk" = reject ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL tamper-linked : [$blk], expected reject"; fi
  bpk=$( { cat "$T/lib2d.proof"; printf ' '; printf '4 5 0 4' | "$T/clp" | sed 's/(use 30)/(use 0)/'; } | "$T/check.exe" )
  if [ "$bpk" = reject ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL tamper-loop : [$bpk], expected reject"; fi
else echo "  (skipped library-linked certifiers — no python3 / gen-lib2d failed)"; fi

# the trust anchor still REJECTS tampered certificates from self-hosted-compiled programs -- delta
# checks the computation, not the compiler that built it:
bad=$(printf '2 3' | "$T/ca" | sed 's/(s (s (s (s (s z)))))/(s (s (s (s z))))/g' | "$T/check.exe")            # claim 2+3 = 4
if [ "$bad" = reject ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL tamper-add : [$bad], expected reject"; fi
badlt=$(printf '2 5' | "$T/clt" | sed 's/(s (s (s (s (s z)))))/(s (s (s (s z))))/g' | "$T/check.exe")         # shrink the < witness
if [ "$badlt" = reject ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL tamper-lt : [$badlt], expected reject"; fi
badcd=$(printf '3 12' | "$T/cd" | sed 's/(s (s (s (s z))))/(s (s (s z)))/' | "$T/check.exe")                  # shrink the cofactor
if [ "$badcd" = reject ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL tamper-divides : [$badcd], expected reject"; fi

echo "self-hosted convergence (the SELF-HOSTED Delta compiler's certifiers, checked by the trust anchor): $PASS confirmed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
echo "SELF-HOSTED CONVERGENCE ✓ — lowermachine-compiled certifiers emit proofs the alpha-rooted trust anchor accepts"
