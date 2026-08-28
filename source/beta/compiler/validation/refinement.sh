#!/usr/bin/env sh
# INSTRUCTION-LEVEL REFINEMENT — certify that Alpha machine code computes the intended function of its inputs,
# proven WITHOUT running it. The lattice already certifies meaning at the SOURCE level (gamma) and RESULTS via
# translation validation; this reaches the bottom rung: the actual bytecode the machine executes.
#
# For each hand-built loop-free arithmetic program, alpha_refinement_check.py (1) symbolically executes the
# tape to a closed-form Peano expression over its inputs, (2) differentially pins that expression to the
# concrete VM (alpha_ref.py) on random inputs, and (3) proves it equals the claimed source meaning for ALL
# inputs — handing the universal goal to the untrusted prover.py and validating its certificate with the trust
# anchor (check.beta). A correct compilation yields a proof-carrying REFINES; a wrong one yields no accepted
# proof. This is the seed of the Cathedral endgame (rungs/*.md): output certifies the compiler, down to the
# instructions. The symbolic executor is UNTRUSTED and checked; nothing here runs in the trusted lineage.
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
cd "$OMEGA_GATE_DIR"
command -v python3 >/dev/null 2>&1 || { echo "refinement: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_BETA_COMPILER}"/artifact_env.sh
. "${OMEGA_PATH_PROOF_KERNEL}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}/$ALPHA_SEED"
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
BC="$T/bc.exe"
stamp_beta_compiler "$BC" >/dev/null 2>&1 || { echo "refinement: lattice bc artifact unavailable"; exit 1; }
stamp_proof_checker "$T/check.exe" >/dev/null 2>&1 || { echo "refinement: checker artifact unavailable"; exit 1; }

echo "instruction-level refinement (alpha machine code provably computes its source meaning, checked without running it):"
python3 "$OMEGA_PATH_BETA_VALIDATION/alpha_refinement_check.py" \
  "$T/check.exe" "$BC" "$ASM"
