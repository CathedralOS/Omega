#!/usr/bin/env sh
# build.sh PROGRAM.beta -> build/tools/beta/PROGRAM.exe
#
# No host compiler: the Beta compiler (the Alpha VM with its compiler tape in
# the tape hole) compiles the program; we stamp that output into a fresh copy
# of the alpha seed. A built program IS the seed with its tape stamped in. The
# per-platform seed + stamping live in ${OMEGA_REPO_ROOT}/tools/bootstrap/alpha/seed_env.sh.
set -e
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
BUILD_DIR="$OMEGA_REPO_ROOT/build/tools/beta"
mkdir -p "$BUILD_DIR"
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
materialize_beta_compiler "$BUILD_DIR/compiler" >/dev/null

SRC=${1:-$OMEGA_REPO_ROOT/tests/beta/compiler/examples/multiply.beta}
NAME=$(basename "$SRC" .beta)
OUT="$BUILD_DIR/$NAME.exe"
TAPE="$BUILD_DIR/$NAME.tape"
TEMP_TAPE="$TAPE.tmp.$$"
trap 'rm -f -- "$TEMP_TAPE"' EXIT HUP INT TERM

# 1. .beta source -> Alpha bytecode, via the Beta compiler running on the seed
"$BUILD_DIR/compiler" < "$SRC" > "$TEMP_TAPE"
mv "$TEMP_TAPE" "$TAPE"
TLEN=$(wc -c < "$TAPE")
[ "$TLEN" -le "$ALPHA_MAX_RAW_TAPE_SIZE" ] || { echo "FAIL: $NAME tape is $TLEN B, exceeds the AlphaBootstrapV2 raw maximum ($ALPHA_MAX_RAW_TAPE_SIZE B)" >&2; exit 1; }

# 2. stamp [4-byte LE length][bytecode] into a fresh copy of the seed
stamp_seed "$TAPE" "$SEED" "$OUT"

echo "built $OUT  ($(wc -c < "$OUT") bytes; $TLEN bytes of program in the seed's hole)"
