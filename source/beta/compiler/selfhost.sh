#!/usr/bin/env sh
# Verify that Beta reconstructs its direct Alpha implementation. The assembler
# assembles its own `assembler.beta` source and must reproduce both the
# canonical raw tape and the tape embedded in the current host container. No
# Rust or host assembler participates.
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
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh"
mkdir -p build
materialize_beta_assembler build/assembler >/dev/null

build/assembler < assembler.beta > build/assembler.tape
L=$(wc -c < build/assembler.tape)
[ "$L" -le "$ALPHA_MAX_RAW_TAPE_SIZE" ] || { echo "FAIL: assembler tape is $L B, exceeds the AlphaBootstrapV2 raw maximum ($ALPHA_MAX_RAW_TAPE_SIZE B)" >&2; exit 1; }

if ! cmp -s build/assembler.tape beta_assembler_bytecode.tape; then
    echo "FAIL: assembler.beta does not reproduce beta_assembler_bytecode.tape" >&2
    exit 1
fi

echo "self-host ✓ — Beta reconstructs its direct Alpha tape byte-identically"
