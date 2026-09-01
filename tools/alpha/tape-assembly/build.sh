#!/usr/bin/env sh
# build.sh PROGRAM.alphaasm -> build/tools/alpha-tape-assembly/PROGRAM.exe
#
# No host compiler: the assembler (the Alpha VM with the Alpha Tape assembler tape in
# its hole) assembles the program to bytecode; we memcpy that into a fresh copy
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
. "$OMEGA_REPO_ROOT/tools/alpha/tape-assembly/artifact_env.sh"
BUILD_DIR="$OMEGA_REPO_ROOT/build/tools/alpha-tape-assembly"
mkdir -p "$BUILD_DIR"
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
materialize_alpha_tape_assembler "$BUILD_DIR/assembler" >/dev/null

SRC=${1:-$OMEGA_REPO_ROOT/tests/alpha/tape-assembly/compiler/examples/multiply.alphaasm}
NAME=$(basename "$SRC" .alphaasm)
OUT="$BUILD_DIR/$NAME.exe"

# 1. .alphaasm text -> bytecode, via the assembler running on the seed
"$BUILD_DIR/assembler" < "$SRC" > "$BUILD_DIR/$NAME.tape"
TLEN=$(wc -c < "$BUILD_DIR/$NAME.tape")
[ "$TLEN" -le "$ALPHA_MAX_RAW_TAPE_SIZE" ] || { echo "FAIL: $NAME tape is $TLEN B, exceeds the AlphaBootstrapV2 raw maximum ($ALPHA_MAX_RAW_TAPE_SIZE B)" >&2; exit 1; }

# 2. stamp [4-byte LE length][bytecode] into a fresh copy of the seed
stamp_seed "$BUILD_DIR/$NAME.tape" "$SEED" "$OUT"

echo "built $OUT  ($(wc -c < "$OUT") bytes; $TLEN bytes of program in the seed's hole)"
