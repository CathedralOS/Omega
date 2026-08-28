#!/usr/bin/env sh
# Construct check.tape directly through Alpha assembler -> Alpha-written cold
# Beta compiler -> Alpha assembler. No accepted Beta compiler participates.
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
trap 'rm -rf "$TMP"' EXIT

"$ASSEMBLER" < "$OMEGA_PATH_BETA_COMPILER/cold-start/bc-alpha.alpha" > "$TMP/cold.tape"
stamp_seed "$TMP/cold.tape" "$SEED" "$TMP/cold" >/dev/null
"$TMP/cold" < "$SCRIPT_DIR/implementations/beta/check.beta" > "$TMP/check.alpha"
"$ASSEMBLER" < "$TMP/check.alpha" > "$TMP/check.tape"
cp "$TMP/check.tape" "$1"
