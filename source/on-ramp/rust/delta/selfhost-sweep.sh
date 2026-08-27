#!/bin/sh
# selfhost-sweep.sh — differential self-hosting sweep across the whole sample corpus.
#
# lowermachine.alp is the Delta-written compiler (it lowers a Delta program to arm64 and
# self-compiles byte-identically; see the FIXPOINT check in test_aarch64.sh). This sweep asserts
# that, for every sample, lowermachine agrees with the trusted Rust-beta reference:
#   (1) LINK   — lowermachine lowers the sample and clang assembles+links it into a Mach-O
#   (2) BEHAVE — the lowermachine-built binary's stdout+exit on a fixed input equals the binary the
#                Rust backend produces for the same source
#
# It is the regression guard for lowermachine's language coverage (the gate's selfhost_tests pin a
# representative subset; this covers everything). It is NOT part of verify-lattice (it compiles every
# sample twice); run it on demand after touching lowermachine.alp.
#
# Runs SEQUENTIALLY with a generous wall-clock alarm rather than a CPU limit: several samples (the
# certify-* certificate emitters) write a byte at a time, and a tight CPU cap truncates their output
# under load and reports spurious divergences. The alarm only backstops a genuine runaway.
#
# Requires clang + codesign + an arm64 host (same as test_aarch64.sh). Exits non-zero on any divergence.
set -e
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "bootstrap paths: repository root not found" >&2; exit 2; }
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
SAMPLES="$OMEGA_PATH_DELTA/samples"
command -v clang    >/dev/null 2>&1 || { echo "selfhost sweep SKIP — no clang"; exit 0; }
command -v codesign >/dev/null 2>&1 || { echo "selfhost sweep SKIP — no codesign"; exit 0; }
cargo build -q 2>/dev/null || { echo "selfhost sweep FAIL — cargo build"; exit 1; }
BIN=./target/debug/delta
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
DELTA_ARCH=aarch64 "$BIN" "$SAMPLES/lowermachine.alp" "$T/lmx" >/dev/null 2>&1 || { echo "selfhost sweep FAIL — building lowermachine"; exit 1; }
INPUT="2 3"                                   # a benign differential input; both sides receive it identically
# Run a binary on $INPUT and emit a comparison key: a BOUNDED stdout prefix plus an exit marker.
# Bounding (head -c) matters because a few samples emit megabytes on this input; comparing the full
# stream would let timing of truncation produce spurious diffs, whereas the deterministic prefix
# still catches any real miscompile. The alarm backstops a no-output infinite loop.
run() { ( { printf '%s' "$INPUT" | perl -e 'alarm 20; exec @ARGV' "$1" 2>/dev/null; printf 'X:%s' "$?"; } | head -c 200000 ) 2>/dev/null; }

n=0; linkfail=0; diff=0
for f in "$SAMPLES"/*.alp; do
  s=$(basename "$f" .alp)
  # the lower* pipeline pieces are components of lowermachine, not standalone programs
  case "$s" in lowermachine|lowersubj|lowertrans|lowerbody) continue;; esac
  DELTA_ARCH=aarch64 "$BIN" "$f" "$T/ref" >/dev/null 2>&1 && codesign -f -s - "$T/ref" 2>/dev/null || { echo "  $s: reference build failed"; continue; }
  "$T/lmx" < "$f" > "$T/g.s" 2>/dev/null
  if ! clang -arch arm64 -o "$T/g" "$T/g.s" 2>"$T/ce"; then
    echo "  $s: LINK FAIL — $(grep -m1 -iE 'undefined|error:' "$T/ce" | sed 's#/[^ ]*/##' | cut -c1-50)"
    linkfail=$((linkfail+1)); continue
  fi
  codesign -f -s - "$T/g" 2>/dev/null
  n=$((n+1))
  set +e
  r=$(run "$T/ref")
  l=$(run "$T/g")
  set -e
  if [ "$r" != "$l" ]; then
    echo "  $s: DIFF (comparison key: ref ${#r}b, lm ${#l}b)"
    diff=$((diff+1))
  fi
done
echo "selfhost sweep: $n samples compared, $linkfail link failures, $diff behavioural divergences"
[ "$linkfail" = 0 ] && [ "$diff" = 0 ] || exit 1
echo "SELFHOST SWEEP ✓ — lowermachine matches the Rust reference across the corpus"
