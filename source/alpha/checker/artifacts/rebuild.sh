#!/usr/bin/env sh
# Deliberately replace the persisted checker artifact through its Gamma edge.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
sh "$SCRIPT_DIR/../construct-artifact.sh" "$SCRIPT_DIR/proof_checker_bytecode.tape"
echo "rebuilt Alpha-rooted checker artifact ($(wc -c < "$SCRIPT_DIR/proof_checker_bytecode.tape" | tr -d ' ') bytes)"
