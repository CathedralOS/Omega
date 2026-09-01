#!/usr/bin/env sh
# Materialize the direct Alpha Tape assembler tape in the selected audited Alpha VM.
# Source tools/bootstrap/paths.sh first.

[ -n "${OMEGA_PATH_ALPHA_TAPE_ASSEMBLY_COMPILER:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "alpha tape assembly artifact: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

materialize_alpha_tape_assembler() {
  ALPHA_TAPE_ASSEMBLER_DEST=$1
  [ -f "$OMEGA_PATH_ALPHA_TAPE_ASSEMBLER_TAPE" ] || {
    echo "alpha tape assembly artifact: missing $OMEGA_PATH_ALPHA_TAPE_ASSEMBLER_TAPE" >&2
    return 2
  }
  stamp_seed "$OMEGA_PATH_ALPHA_TAPE_ASSEMBLER_TAPE" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$ALPHA_TAPE_ASSEMBLER_DEST"
}
