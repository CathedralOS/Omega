#!/usr/bin/env sh
# Shared loader for the platform-independent, lattice-built Beta compiler tape.
# Source tools/bootstrap/paths.sh first. The function stamps the tape into the audited
# Alpha seed selected for the host; no Rust producer participates.

[ -n "${OMEGA_PATH_BETA:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "beta artifact: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_PATH_ALPHA/seed_env.sh"
BETA_COMPILER_TAPE="$OMEGA_PATH_BETA/artifacts/bc.tape"
export BETA_COMPILER_TAPE

stamp_beta_compiler() {
  BETA_COMPILER_DEST=$1
  [ -f "$BETA_COMPILER_TAPE" ] || {
    echo "beta artifact: missing $BETA_COMPILER_TAPE" >&2
    return 2
  }
  stamp_seed "$BETA_COMPILER_TAPE" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$BETA_COMPILER_DEST"
}
