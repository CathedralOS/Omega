#!/usr/bin/env sh
# ./build.sh PROG.beta  ->  build/PROG.exe   (a standalone seed exe)
#
# The chain: beta-lang compiles .beta -> Alpha assembly; the assembler (${OMEGA_PATH_ALPHA_ASSEMBLER})
# lowers assembly -> a tape; the tape is stamped into the alpha seed. beta-lang
# is the throwaway Rust on-ramp for the Beta compiler. Per-platform seed +
# stamping live in ${OMEGA_PATH_ALPHA}/seed_env.sh.
set -e
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
mkdir -p build
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED

SRC=${1:-examples/answer.beta}
NAME=$(basename "$SRC" .beta)

cargo run --quiet < "$SRC"            > "build/$NAME.asm"    # .beta -> Alpha assembly
"$ASM"            < "build/$NAME.asm" > "build/$NAME.tape"   # assembly -> tape
L=$(wc -c < "build/$NAME.tape")
[ $((L + 4)) -le "$HOLE_SIZE" ] || { echo "FAIL: $NAME tape is $L B, exceeds the seed's tape hole ($HOLE_SIZE B)" >&2; exit 1; }

stamp_seed "build/$NAME.tape" "$SEED" "build/$NAME.exe"
echo "built build/$NAME.exe  (beta -> asm -> tape -> stamp)"
