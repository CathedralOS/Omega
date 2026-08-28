#!/usr/bin/env sh
# Verify an already-observed lower-rooted Delta assembly receipt.
set -eu

if [ "$#" -ne 16 ]; then
  echo "usage: $0 RECEIPT ASSEMBLER_TAPE TRANSLATOR_TAPE INTERPRETER_TAPE TEMPLATE GAMMA ELAB_OBS ELAB_ERR EXEC0_OBS EXEC0_RAW EXEC0_ASM EXEC0_ERR EXEC1_OBS EXEC1_RAW EXEC1_ASM EXEC1_ERR" >&2
  exit 2
fi

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "Delta assembly publication V1: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$OMEGA_PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"

RECEIPT=$1
shift
VERIFY=$OMEGA_PATH_DELTA/lower_rooted_assembly_publication_v1.py
MANIFEST=$OMEGA_PATH_DELTA/source-closures/canonical-compiler-v1.json
LOCATIONS=$OMEGA_PATH_DELTA/source-closures/canonical-compiler-v1.locations.json

python3 -B "$VERIFY" verify "$RECEIPT" "$MANIFEST" "$LOCATIONS" \
  "$@" "delta=$OMEGA_PATH_DELTA"

echo "Delta lower-rooted assembly publication V1 PASS — exact source/template/closed-Gamma custody; two agreeing executions; strict assembly"
