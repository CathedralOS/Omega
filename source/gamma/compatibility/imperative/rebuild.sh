#!/usr/bin/env sh
# Rebuild gamma_x64_windows.exe = the alpha seed with gamma's tape stamped into its hole.
# gamma.alpha is assembled by beta (the rung below). No Rust, no Python.
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
SEED="${OMEGA_PATH_ALPHA}"/alpha_x64_windows.exe
mkdir -p build
"${OMEGA_PATH_ALPHA_ASSEMBLER}"/beta_x64_windows.exe < gamma.alpha > build/gamma.tape
L=$(wc -c < build/gamma.tape)
[ $((L + 4)) -le 32768 ] || { echo "FAIL: gamma tape is $L B, exceeds the seed's 32 KB hole" >&2; exit 1; }
cp "$SEED" gamma_x64_windows.exe
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) $(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
    | dd of=gamma_x64_windows.exe bs=1 seek=5120 conv=notrunc status=none
dd if=build/gamma.tape of=gamma_x64_windows.exe bs=1 seek=5124 conv=notrunc status=none
echo "rebuilt gamma_x64_windows.exe ($L-byte tape)"
