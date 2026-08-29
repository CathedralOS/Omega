#!/usr/bin/env sh
# Rebuild the canonical Beta compiler tape directly from its Alpha source. With
# --check, compare the reconstruction to the persisted artifact without changing it.
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

"$ASSEMBLER" < "$OMEGA_PATH_BETA_COMPILER_SOURCE" > "$TMP/compiler.tape"

ARTIFACT="$OMEGA_PATH_BETA_COMPILER_TAPE"
if [ "$MODE" = "--check" ]; then
  cmp "$TMP/compiler.tape" "$ARTIFACT"
  echo "Alpha-written Beta compiler matches the persisted artifact ($(wc -c < "$ARTIFACT" | tr -d ' ') bytes)"
else
  cp "$TMP/compiler.tape" "$ARTIFACT"
  echo "rebuilt canonical Beta compiler artifact ($(wc -c < "$ARTIFACT" | tr -d ' ') bytes)"
fi
