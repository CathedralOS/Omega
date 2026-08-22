#!/usr/bin/env sh
# Verify beta self-hosts: the assembler assembles its own source (assembler.alpha)
# and reproduces, byte-for-byte, the bytecode already embedded in the committed
# beta seed. No Rust. The comparison is on the TAPE (the program bytecode) rather
# than the whole binary, so it is deterministic on macOS too (where the OS-imposed
# code signature blob is non-reproducible — the program bytes are the guarantee).
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

"./$BETA_SEED" < assembler.alpha > build/assembler.tape
L=$(wc -c < build/assembler.tape)
[ $((L + 4)) -le "$HOLE_SIZE" ] || { echo "FAIL: assembler tape is $L B, exceeds the seed's tape hole ($HOLE_SIZE B)" >&2; exit 1; }

# the canonical bytecode embedded in the committed beta seed, as [4-byte len][tape]
tape_in_seed "$BETA_SEED" > build/embedded.tape
# our freshly-assembled bytecode, framed the same way
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) $(((L >> 16) & 255)) $(((L >> 24) & 255)))" > build/fresh.tape
cat build/assembler.tape >> build/fresh.tape

if cmp -s build/embedded.tape build/fresh.tape; then
    echo "self-host ✓ — beta rebuilds its own bytecode from assembler.alpha, byte-identical, no Rust"
else
    echo "FAIL: beta does not reproduce its own bytecode" >&2; exit 1
fi
