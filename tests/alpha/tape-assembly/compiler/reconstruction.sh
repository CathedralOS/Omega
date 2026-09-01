#!/usr/bin/env sh
# Verify that Alpha Tape Assembly reconstructs its direct Alpha implementation. The assembler
# assembles its own `assembler.alphaasm` source and must reproduce both the
# canonical raw tape and the tape embedded in the current host container. No
# Rust or host assembler participates.
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
TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT
materialize_alpha_tape_assembler "$TMP/assembler" >/dev/null

"$TMP/assembler" < "$OMEGA_PATH_ALPHA_TAPE_ASSEMBLER_SOURCE" > "$TMP/assembler.tape"
L=$(wc -c < "$TMP/assembler.tape")
[ "$L" -le "$ALPHA_MAX_RAW_TAPE_SIZE" ] || { echo "FAIL: assembler tape is $L B, exceeds the AlphaBootstrapV2 raw maximum ($ALPHA_MAX_RAW_TAPE_SIZE B)" >&2; exit 1; }

if ! cmp -s "$TMP/assembler.tape" "$OMEGA_PATH_ALPHA_TAPE_ASSEMBLER_TAPE"; then
    echo "FAIL: assembler.alphaasm does not reproduce alpha_tape_assembler_bytecode.tape" >&2
    exit 1
fi

echo "reconstruction — Alpha Tape Assembly reconstructs its direct Alpha tape byte-identically"
