#!/usr/bin/env sh
# Materialize the direct Beta assembler tape in the selected audited Alpha VM.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_BETA_COMPILER:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "beta artifact: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

materialize_beta_assembler() {
  BETA_ASSEMBLER_DEST=$1
  [ -f "$OMEGA_PATH_BETA_ASSEMBLER_TAPE" ] || {
    echo "beta artifact: missing $OMEGA_PATH_BETA_ASSEMBLER_TAPE" >&2
    return 2
  }
  stamp_seed "$OMEGA_PATH_BETA_ASSEMBLER_TAPE" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$BETA_ASSEMBLER_DEST"
}
