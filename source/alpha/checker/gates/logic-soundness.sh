#!/usr/bin/env sh
# LOGIC SOUNDNESS SEAM -- the propositional-logic pillar bridged to classical TRUTH.
#
# The fourth operational seam (after semantics-diamond=equality, induction-soundness=universals,
# predicate-soundness=inductive predicates). implementations/beta/check.beta's logic is INTUITIONISTIC, so everything it
# proves is CLASSICALLY valid: for each propositional proof it accepts, an independent truth-table
# oracle must find the goal a TAUTOLOGY, and a perturbed genuine NON-tautology must be REJECTED. Two
# independent routes -- a kernel typing derivation and a semantic decision -- agreeing is evidence the
# checker's logic is sound (not a proof; the theorem is the open problem). Needs python3.
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
. "$OMEGA_PATH_ALPHA_CHECKER/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_ALPHA_CHECKER"
command -v python3 >/dev/null 2>&1 || { echo "logic-soundness: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }
python3 corpus/fuzz/logic-soundness.py "$T/check.exe" "${1:-100}"
