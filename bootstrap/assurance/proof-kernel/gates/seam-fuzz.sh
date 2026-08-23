#!/usr/bin/env sh
# SEAM FUZZER — broad random coverage of the proof/meaning soundness seam (semantics-diamond.sh is the
# hand-picked version). The checker's DEFINITIONAL equality (implementations/beta/eq.beta: normalize both sides) and the
# reference interpreter's OPERATIONAL evaluation (interp.beta, running gamma's own plus/mult) must
# agree on whether two closed Peano terms are equal -- two independent routes to "e1 = e2". This runs
# that comparison over MANY randomly generated +/* expressions instead of a curated handful: for each
# it checks both `E = value(E)` (must agree "equal") and `E = value(E)+1` (must agree "differ"). A
# single disagreement would be a soundness break at the seam (not a proof of the open theorem, but the
# broadest bounded evidence for it). Deterministic (fixed seed). Needs python3; skips cleanly without.
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
. "$OMEGA_PATH_BETA/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
command -v python3 >/dev/null 2>&1 || { echo "seam fuzz: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b implementations/beta/eq.beta "$T/eq.exe"                  || { echo "build implementations/beta/eq.beta failed"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
python3 corpus/fuzz/seam-fuzz.py "$T/eq.exe" "$T/interp.exe" "${1:-120}"
