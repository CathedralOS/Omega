#!/usr/bin/env sh
# REFERENCE-ROUTE CONVERGENCE -- proof-carrying computation on the MEANING route.
#
# convergence.sh / convergence-selfhost.sh run a certifier as NATIVE code (the fast route: the
# delta-rust backend, or the self-hosted lowermachine) and have the trust anchor check the emitted
# certificate. This runs the same certifier down the SLOW, MEANING-DEFINING route instead: the delta
# program is translated to gamma (DELTA_EMIT=gamma) and EXECUTED by the Rust-free reference interpreter
# `interp.beta` -- the rung's "meaning" -- which emits the certificate, and the trust anchor `check.beta`
# (also Rust-free, alpha-rooted) accepts it. So the whole proof-carrying loop -- COMPUTE then CHECK --
# runs on lattice artifacts via the reference interpreter, directly answering the architecture's open
# question "the smallest end-to-end slice ... with the checker run down the reference route". A WRONG
# computation's certificate is REJECTED here too, so the loop is meaningful, not vacuous.
#
# The delta->gamma translator is itself Rust, but the delta-meaning diamond independently certifies
# that the gamma route reproduces native execution byte-for-byte -- so this leans on a cross-checked step.
# Skips cleanly off macOS arm64 or without the toolchain. Small-number certifiers only (a large
# certificate's unary numerals exhaust interp's arena).
# No `set -e`: check.beta/interp.beta exit with the alpha VM's result byte (a non-zero code captured
# inside `v=$(…)`), so judge by stdout and guard each build step explicitly instead.
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
cd "$OMEGA_GATE_DIR"
SAMPLES="$OMEGA_PATH_DELTA/samples"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "convergence-reference SKIP -- not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "convergence-reference SKIP -- no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
. "${OMEGA_PATH_BETA}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
stamp_beta_compiler "$T/bc.exe" >/dev/null || { echo "convergence-reference FAIL -- Beta compiler artifact"; exit 1; }
b() { "$T/bc.exe" < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b "${OMEGA_PATH_PROOF_KERNEL}"/implementations/beta/check.beta "$T/check.exe"   || { echo "convergence-reference FAIL -- build check.beta"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "convergence-reference FAIL -- build interp.beta"; exit 1; }
cargo build -q 2>/dev/null || { echo "convergence-reference FAIL -- cargo build"; exit 1; }

PASS=0; FAIL=0
# ref SAMPLE "in-bytes" EXPECT : translate the certifier to gamma, RUN it on interp.beta (the reference
# interpreter) with that stdin, decode the emitted certificate from interp's output list, and require
# check.beta's verdict to be EXPECT. Self-contained certifiers only (no banked library to prepend).
ref() {
  g=$(DELTA_GAMMA_INPUT="$2" DELTA_EMIT=gamma ./target/debug/delta "$SAMPLES/$1.alp" 2>/dev/null)
  [ -n "$g" ] || { FAIL=$((FAIL+1)); echo "  FAIL $1 : no gamma emitted"; return; }
  cert=$(printf '%s\n' "$g" | "$T/interp.exe" 2>/dev/null | grep -oE '[0-9]+' | awk '{printf "%c",$1}')
  v=$(printf '%s' "$cert" | "$T/check.exe")
  if [ "$v" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : check.beta returned [$v], expected $3"; fi
}

# diverse cert kinds, each computed by the reference interpreter and accepted by the trust anchor:
ref certify-add     "50 32 51"          accept  # '2 3'   -> equality / refl (a+b)
ref certify-product "50 32 51 32 53"    accept  # '2 3 5' -> inductive predicate ProdIs (a list's product)
ref certify-member  "53 32 53 32 54 32 55" accept  # '5 5 6 7' -> inductive predicate Mem (list membership)
ref certify-divides "51 32 49 50"       accept  # '3 12'  -> existential (3 divides 12)
ref certify-wrong   "50 32 51"          reject  # NEGATIVE CONTROL: the buggy 'a+b = a+b+1' cert is rejected

echo "reference-route convergence (certifier RUN on interp.beta; cert checked by check.beta): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
