#!/usr/bin/env sh
# Materialize and invoke the downgraded concatenative Delta-to-Gamma compiler.
# Source tools/bootstrap/paths.sh, beta/artifact_env.sh, and
# gamma/artifact_env.sh first.

[ -n "${OMEGA_PATH_CONCATENATIVE_DELTA_COMPILER_SOURCE:-}" ] && [ -n "${OMEGA_PATH_ALPHA:-}" ] || {
  echo "Delta artifact: source tools/bootstrap/paths.sh first" >&2
  return 2 2>/dev/null || exit 2
}

materialize_delta_compiler() {
  DELTA_COMPILER_DEST=$1
  GAMMA_COMPILER_EXE=$2
  BETA_COMPILER_EXE=$3
  DELTA_COMPILER_TAPE_TEMP=${DELTA_COMPILER_DEST}.tape.tmp

  if ! compile_gamma_source_to_tape "$GAMMA_COMPILER_EXE" "$BETA_COMPILER_EXE" \
      "$OMEGA_PATH_CONCATENATIVE_DELTA_COMPILER_SOURCE" "$DELTA_COMPILER_TAPE_TEMP"; then
    rm -f -- "$DELTA_COMPILER_TAPE_TEMP"
    return 1
  fi
  if ! stamp_seed "$DELTA_COMPILER_TAPE_TEMP" \
      "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$DELTA_COMPILER_DEST"; then
    rm -f -- "$DELTA_COMPILER_TAPE_TEMP" "$DELTA_COMPILER_DEST"
    return 1
  fi
  rm -f -- "$DELTA_COMPILER_TAPE_TEMP"
}

compile_delta_source_to_tape() {
  DELTA_COMPILER_EXE=$1
  GAMMA_COMPILER_EXE=$2
  BETA_COMPILER_EXE=$3
  DELTA_SOURCE=$4
  DELTA_TAPE_DEST=$5
  DELTA_GAMMA_TEMP=${DELTA_TAPE_DEST}.gamma.tmp

  if ! "$DELTA_COMPILER_EXE" < "$DELTA_SOURCE" > "$DELTA_GAMMA_TEMP"; then
    rm -f -- "$DELTA_GAMMA_TEMP"
    return 1
  fi
  if ! compile_gamma_source_to_tape "$GAMMA_COMPILER_EXE" \
      "$BETA_COMPILER_EXE" "$DELTA_GAMMA_TEMP" "$DELTA_TAPE_DEST"; then
    rm -f -- "$DELTA_GAMMA_TEMP" "$DELTA_TAPE_DEST"
    return 1
  fi
  rm -f -- "$DELTA_GAMMA_TEMP"
}
