#!/usr/bin/env sh
# Construct proof_checker_bytecode.tape with the canonical Gamma compiler tape.
# No frontend or host compiler state participates in this construction step.
set -eu

[ "$#" -eq 1 ] || { echo "usage: $0 OUTPUT_TAPE" >&2; exit 2; }
TOOL_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$TOOL_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT

stamp_seed "$OMEGA_PATH_GAMMA_COMPILER_TAPE" "$SEED" "$TMP/compiler" >/dev/null
"$TMP/compiler" < "$OMEGA_PATH_ALPHA_CHECKER/implementations/gamma/check.gamma" > "$TMP/proof_checker_bytecode.tape"
cp "$TMP/proof_checker_bytecode.tape" "$1"
