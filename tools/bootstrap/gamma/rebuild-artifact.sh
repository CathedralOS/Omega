#!/usr/bin/env sh
# Rebuild the canonical Gamma compiler tape directly from its Beta source. With
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
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT
materialize_beta_assembler "$TMP/assembler" >/dev/null
ASSEMBLER="$TMP/assembler"

"$ASSEMBLER" < "$OMEGA_PATH_GAMMA_COMPILER_SOURCE" > "$TMP/compiler.tape"

ARTIFACT="$OMEGA_PATH_GAMMA_COMPILER_TAPE"
if [ "$MODE" = "--check" ]; then
  cmp "$TMP/compiler.tape" "$ARTIFACT"
  echo "Beta-written Gamma compiler matches the persisted artifact ($(wc -c < "$ARTIFACT" | tr -d ' ') bytes)"
else
  cp "$TMP/compiler.tape" "$ARTIFACT"
  echo "rebuilt canonical Gamma compiler artifact ($(wc -c < "$ARTIFACT" | tr -d ' ') bytes)"
fi
