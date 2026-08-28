#!/usr/bin/env sh
# LOGIC-DIAMOND FUZZER -- broad random coverage of the two/three trust-anchor checkers agreeing on
# PROPOSITIONAL LOGIC proofs (the ->/&/+/bot intro+elim rules). seam-fuzz fuzzes the reducer and
# checker-diamond-fuzz the equality conversion; this fuzzes the logical typing rules, cross-checked across
# all checkers only at the ~25 curated checker-diamond.sh cases otherwise. Generates hundreds of valid
# tautology-schema proofs at random atoms and requires every checker to ACCEPT each against its goal and
# REJECT it against a perturbed goal. A disagreement is a bug in one checker's logic. Needs python3.
set -e
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
. "$OMEGA_PATH_PROOF_KERNEL/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
command -v python3 >/dev/null 2>&1 || { echo "logic-diamond fuzz: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
# Third oracle: the type-checked checker, mechanically type-erased to what the interpreter runs.
TYPED=""
if python3 "${OMEGA_PATH_PROOF_KERNEL}"/tools/erase-gamma-types.py < "${OMEGA_PATH_PROOF_KERNEL}"/implementations/gamma/checker_typed.gamma > "$T/erased.gamma" 2>/dev/null; then
  TYPED="$T/erased.gamma"
fi
python3 corpus/fuzz/logic-diamond-fuzz.py "$T/check.exe" "$T/interp.exe" "${OMEGA_PATH_PROOF_KERNEL}"/implementations/gamma/checker.gamma "$TYPED" "${1:-60}"
