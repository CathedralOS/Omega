#!/usr/bin/env sh
# Deliberately replace the persisted checker artifact through its Gamma edge.
set -eu

TOOL_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$TOOL_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
sh "$TOOL_DIR/construct-artifact.sh" "$OMEGA_PATH_ALPHA_CHECKER/artifacts/proof_checker_bytecode.tape"
echo "rebuilt Alpha-rooted checker artifact ($(wc -c < "$OMEGA_PATH_ALPHA_CHECKER/artifacts/proof_checker_bytecode.tape" | tr -d ' ') bytes)"
