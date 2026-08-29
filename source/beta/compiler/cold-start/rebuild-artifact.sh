#!/usr/bin/env sh
# Rebuild the fixed-point bc tape through Alpha -> Beta only. With --check,
# compare the reconstruction to the persisted artifact without changing it.
set -eu

MODE=${1:-install}
case "$MODE" in
  install|--check) ;;
  *)
    echo "usage: rebuild-artifact.sh [--check]" >&2
    exit 2
    ;;
esac

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

"$ASSEMBLER" < "$SCRIPT_DIR/bc-alpha.alpha" > "$TMP/cold.tape"
stamp_seed "$TMP/cold.tape" "$SEED" "$TMP/cold" >/dev/null
"$TMP/cold" < "$OMEGA_PATH_BETA_COMPILER/bc.beta" > "$TMP/bootstrap.tape"
stamp_seed "$TMP/bootstrap.tape" "$SEED" "$TMP/bootstrap-bc" >/dev/null
"$TMP/bootstrap-bc" < "$OMEGA_PATH_BETA_COMPILER/bc.beta" > "$TMP/fixed.alpha"
"$ASSEMBLER" < "$TMP/fixed.alpha" > "$TMP/fixed.tape"

ARTIFACT="$OMEGA_PATH_BETA_COMPILER/artifacts/beta_compiler_bytecode.tape"
if [ "$MODE" = "--check" ]; then
  cmp "$TMP/fixed.tape" "$ARTIFACT"
  echo "Alpha-rooted bc construction matches the persisted artifact ($(wc -c < "$ARTIFACT" | tr -d ' ') bytes)"
else
  mkdir -p "$OMEGA_PATH_BETA_COMPILER/artifacts"
  cp "$TMP/fixed.tape" "$ARTIFACT"
  echo "rebuilt lattice bc artifact ($(wc -c < "$ARTIFACT" | tr -d ' ') bytes)"
fi
