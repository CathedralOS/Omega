#!/usr/bin/env sh
# Rooted ordinary-FOL seam for the non-lockstep trace architecture selected by
# BETA-COMPILER-FOL-REFINEMENT.  This does not claim bc.beta admission.
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
. "$OMEGA_PATH_ALPHA/seed_env.sh" || exit $?

OMEGA_TRACE_TMP=$(mktemp -d)
trap 'rm -rf "$OMEGA_TRACE_TMP"' EXIT
stamp_beta_compiler "$OMEGA_TRACE_TMP/bc.exe" >/dev/null || exit $?
stamp_proof_checker "$OMEGA_TRACE_TMP/check.exe" >/dev/null || exit $?
"$OMEGA_TRACE_TMP/bc.exe" < "$OMEGA_PATH_GAMMA/interp.beta" \
  > "$OMEGA_TRACE_TMP/interp.alpha" 2>/dev/null || exit $?
"$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED" \
  < "$OMEGA_TRACE_TMP/interp.alpha" \
  > "$OMEGA_TRACE_TMP/gamma_interpreter_bytecode.tape" 2>/dev/null || exit $?
stamp_seed "$OMEGA_TRACE_TMP/gamma_interpreter_bytecode.tape" \
  "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
  "$OMEGA_TRACE_TMP/interp.exe" >/dev/null 2>&1 || exit $?

python3 "$OMEGA_GATE_DIR/trace_refinement_seam.py" \
  "$OMEGA_TRACE_TMP/check.exe" \
  "$OMEGA_PATH_ALPHA_CHECKER/implementations/reference/check_ref.py" \
  "$OMEGA_PATH_ALPHA_CHECKER/tools/elab.py" \
  "$OMEGA_PATH_ALPHA_CHECKER/tools/refcert_to_gamma.py" \
  "$OMEGA_TRACE_TMP/interp.exe" \
  "$OMEGA_PATH_ALPHA_CHECKER/implementations/gamma/checker.gamma" \
  "$OMEGA_GATE_DIR"
