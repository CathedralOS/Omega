#!/usr/bin/env sh
# Prove the persisted bc artifact is rebuilt by the Alpha-rooted chain, reaches
# a fixed point, and passes the complete retained Beta language corpus.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$SCRIPT_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"

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
cmp "$TMP/fixed.tape" "$BETA_COMPILER_TAPE"

stamp_beta_compiler "$TMP/bc"
"$TMP/bc" < "$OMEGA_PATH_BETA/bc.beta" > "$TMP/reproduced.alpha"
"$ASSEMBLER" < "$TMP/reproduced.alpha" > "$TMP/reproduced.tape"
cmp "$TMP/fixed.alpha" "$TMP/reproduced.alpha"
cmp "$BETA_COMPILER_TAPE" "$TMP/reproduced.tape"

BETA_COMPILER_EXE="$TMP/bc" sh "$OMEGA_PATH_BETA/test.sh"
echo "Alpha-rooted bc full source + fixed point + corpus OK ($(wc -c < "$BETA_COMPILER_TAPE" | tr -d ' ')-byte artifact)"
