#!/usr/bin/env sh
# Materialize and invoke the canonical Gamma-to-Beta compiler.
# Source tools/bootstrap/paths.sh and beta/artifact_env.sh first.

[ -n "${OMEGA_PATH_GAMMA_COMPILER_TAPE:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Gamma artifact: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

materialize_gamma_compiler() {
  GAMMA_COMPILER_DEST=$1
  [ -f "$OMEGA_PATH_GAMMA_COMPILER_TAPE" ] || {
    echo "Gamma artifact: missing $OMEGA_PATH_GAMMA_COMPILER_TAPE" >&2
    return 2
  }
  stamp_seed "$OMEGA_PATH_GAMMA_COMPILER_TAPE" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$GAMMA_COMPILER_DEST"
}

compile_gamma_source_to_tape() {
  GAMMA_COMPILER_EXE=$1
  BETA_COMPILER_EXE=$2
  GAMMA_SOURCE=$3
  GAMMA_TAPE_DEST=$4
  GAMMA_BETA_TEMP=${GAMMA_TAPE_DEST}.beta.tmp

  if ! "$GAMMA_COMPILER_EXE" < "$GAMMA_SOURCE" > "$GAMMA_BETA_TEMP"; then
    rm -f -- "$GAMMA_BETA_TEMP"
    return 1
  fi
  if ! "$BETA_COMPILER_EXE" < "$GAMMA_BETA_TEMP" > "$GAMMA_TAPE_DEST"; then
    rm -f -- "$GAMMA_BETA_TEMP" "$GAMMA_TAPE_DEST"
    return 1
  fi
  rm -f -- "$GAMMA_BETA_TEMP"
}
