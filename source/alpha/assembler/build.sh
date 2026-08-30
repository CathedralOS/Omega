#!/usr/bin/env sh
# ./build.sh PROGRAM.alpha   ->   build/PROGRAM.exe   (a standalone seed exe)
#
# Pure alpha, no Rust: the assembler (the alpha seed with the assembler tape in
# its hole) assembles the program to bytecode; we memcpy that into a fresh copy
# of the alpha seed. A built program IS the seed with its tape stamped in. The
# per-platform seed + stamping live in ${OMEGA_PATH_ALPHA}/seed_env.sh.
set -e
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
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
mkdir -p build
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED

SRC=${1:-examples/multiply.alpha}
NAME=$(basename "$SRC" .alpha)
OUT="build/$NAME.exe"

# 1. .alpha text -> bytecode, via the assembler running on the seed
"./$BETA_SEED" < "$SRC" > "build/$NAME.tape"
TLEN=$(wc -c < "build/$NAME.tape")
[ "$TLEN" -le "$ALPHA_MAX_RAW_TAPE_SIZE" ] || { echo "FAIL: $NAME tape is $TLEN B, exceeds the AlphaBootstrapV2 raw maximum ($ALPHA_MAX_RAW_TAPE_SIZE B)" >&2; exit 1; }

# 2. stamp [4-byte LE length][bytecode] into a fresh copy of the seed
stamp_seed "build/$NAME.tape" "$SEED" "$OUT"

echo "built $OUT  ($(wc -c < "$OUT") bytes; $TLEN bytes of program in the seed's hole)"
