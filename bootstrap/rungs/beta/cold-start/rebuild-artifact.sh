#!/usr/bin/env sh
# Rebuild the persisted fixed-point bc tape through Alpha -> Beta only.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$SCRIPT_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"

ASSEMBLER="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

"$ASSEMBLER" < "$SCRIPT_DIR/bc-alpha.alpha" > "$TMP/cold.tape"
stamp_seed "$TMP/cold.tape" "$SEED" "$TMP/cold" >/dev/null
"$TMP/cold" < "$OMEGA_PATH_BETA/bc.beta" > "$TMP/bootstrap.alpha"
"$ASSEMBLER" < "$TMP/bootstrap.alpha" > "$TMP/bootstrap.tape"
stamp_seed "$TMP/bootstrap.tape" "$SEED" "$TMP/bootstrap-bc" >/dev/null
"$TMP/bootstrap-bc" < "$OMEGA_PATH_BETA/bc.beta" > "$TMP/fixed.alpha"
"$ASSEMBLER" < "$TMP/fixed.alpha" > "$TMP/fixed.tape"

mkdir -p "$OMEGA_PATH_BETA/artifacts"
cp "$TMP/fixed.tape" "$OMEGA_PATH_BETA/artifacts/bc.tape"
echo "rebuilt lattice bc artifact ($(wc -c < "$OMEGA_PATH_BETA/artifacts/bc.tape" | tr -d ' ') bytes)"
