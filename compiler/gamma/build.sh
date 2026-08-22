#!/usr/bin/env sh
# ./build.sh PROG.gamma   ->   build/PROG.exe   (a standalone Windows exe)
#
# The chain: gamma compiles .gamma source -> alpha assembly; beta lowers assembly -> a
# tape; the tape is memcpy'd into the alpha seed. Each rung targets the one below.
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
mkdir -p build
SEED="${OMEGA_PATH_ALPHA}"/alpha_x64_windows.exe

SRC=${1:-examples/answer.gamma}
NAME=$(basename "$SRC" .gamma)

./gamma_x64_windows.exe        < "$SRC"            > "build/$NAME.asm"    # .gamma -> assembly
"${OMEGA_PATH_BETA_ASSEMBLER}"/beta_x64_windows.exe   < "build/$NAME.asm" > "build/$NAME.tape"   # assembly -> tape
L=$(wc -c < "build/$NAME.tape")
[ $((L + 4)) -le 32768 ] || { echo "FAIL: $NAME tape is $L B, exceeds the seed's 32 KB hole" >&2; exit 1; }

cp "$SEED" "build/$NAME.exe"
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) $(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
    | dd of="build/$NAME.exe" bs=1 seek=5120 conv=notrunc status=none
dd if="build/$NAME.tape" of="build/$NAME.exe" bs=1 seek=5124 conv=notrunc status=none
echo "built build/$NAME.exe  (gamma -> beta -> stamp)"
