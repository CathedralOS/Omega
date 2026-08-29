#!/usr/bin/env sh
# Construct proof_checker_bytecode.tape directly through the Alpha assembler
# and canonical Alpha-written Beta compiler. No Beta self-host or
# post-compilation assembler stage participates.
set -eu

[ "$#" -eq 1 ] || { echo "usage: $0 OUTPUT_TAPE" >&2; exit 2; }
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$SCRIPT_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"

ASSEMBLER="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
TMP=$(mktemp -d)
trap 'trash "$TMP"' EXIT

"$ASSEMBLER" < "$OMEGA_PATH_BETA_COMPILER/beta_compiler.alpha" > "$TMP/compiler.tape"
stamp_seed "$TMP/compiler.tape" "$SEED" "$TMP/compiler" >/dev/null
"$TMP/compiler" < "$SCRIPT_DIR/implementations/beta/check.beta" > "$TMP/proof_checker_bytecode.tape"
cp "$TMP/proof_checker_bytecode.tape" "$1"
