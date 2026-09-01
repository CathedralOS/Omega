#!/usr/bin/env sh
# Loader for the platform-independent checker tape constructed through Gamma.
# Source tools/lattice/paths.sh first.

[ -n "${OMEGA_PATH_ALPHA_CHECKER:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "checker artifact: source tools/lattice/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_PATH_ALPHA/seed_env.sh"
PROOF_CHECKER_TAPE="$OMEGA_PATH_ALPHA_CHECKER/artifacts/proof_checker_bytecode.tape"
export PROOF_CHECKER_TAPE

stamp_proof_checker() {
  PROOF_CHECKER_DEST=$1
  [ -f "$PROOF_CHECKER_TAPE" ] || {
    echo "checker artifact: missing $PROOF_CHECKER_TAPE" >&2
    return 2
  }
  stamp_seed "$PROOF_CHECKER_TAPE" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$PROOF_CHECKER_DEST"
}
