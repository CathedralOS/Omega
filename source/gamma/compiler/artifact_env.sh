#!/usr/bin/env sh
# Shared loader for the canonical platform-independent Gamma compiler tape.
# Source tools/lattice/paths.sh first. The function stamps the Beta-written
# compiler tape into the audited Alpha seed selected for the host.

[ -n "${OMEGA_PATH_BETA:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "gamma artifact: source tools/lattice/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_PATH_ALPHA/seed_env.sh"
GAMMA_COMPILER_TAPE="$OMEGA_PATH_GAMMA_COMPILER_TAPE"
export GAMMA_COMPILER_TAPE

stamp_gamma_compiler() {
  GAMMA_COMPILER_DEST=$1
  [ -f "$GAMMA_COMPILER_TAPE" ] || {
    echo "gamma artifact: missing $GAMMA_COMPILER_TAPE" >&2
    return 2
  }
  stamp_seed "$GAMMA_COMPILER_TAPE" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$GAMMA_COMPILER_DEST"
}
