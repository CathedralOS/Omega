#!/usr/bin/env sh
# Pin the canonical Alpha ownership paths and their temporary legacy aliases.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd -P)
. "$SCRIPT_DIR/paths.sh"

fail() {
  echo "bootstrap paths FAIL — $*" >&2
  exit 1
}

physical_dir() {
  CDPATH= cd -- "$1" 2>/dev/null && pwd -P
}

[ "$OMEGA_PATH_ALPHA" = "$OMEGA_REPO_ROOT/bootstrap/rungs/alpha" ] ||
  fail "Alpha owner is $OMEGA_PATH_ALPHA"
[ "$OMEGA_PATH_ALPHA_ASSEMBLER" = "$OMEGA_PATH_ALPHA/assembler" ] ||
  fail "Alpha assembler owner is $OMEGA_PATH_ALPHA_ASSEMBLER"
[ "$OMEGA_PATH_BETA_ASSEMBLER" = "$OMEGA_PATH_ALPHA_ASSEMBLER" ] ||
  fail "historical assembler variable does not resolve to the Alpha owner"

[ -L "$OMEGA_REPO_ROOT/compiler/alpha" ] ||
  fail "compiler/alpha is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/beta" ] ||
  fail "compiler/beta is not a temporary compatibility symlink"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/alpha")" = "$(physical_dir "$OMEGA_PATH_ALPHA")" ] ||
  fail "compiler/alpha does not resolve to the Alpha owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/beta")" = "$(physical_dir "$OMEGA_PATH_ALPHA_ASSEMBLER")" ] ||
  fail "compiler/beta does not resolve to the Alpha assembler owner"

[ "$(omega_bootstrap_path alpha)" = "$OMEGA_PATH_ALPHA" ] ||
  fail "alpha role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path alpha-assembler)" = "$OMEGA_PATH_ALPHA_ASSEMBLER" ] ||
  fail "alpha-assembler role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path beta-assembler)" = "$OMEGA_PATH_ALPHA_ASSEMBLER" ] ||
  fail "beta-assembler compatibility role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path beta)" = "$OMEGA_PATH_ALPHA_ASSEMBLER" ] ||
  fail "beta compatibility role lookup disagrees with the manifest"

echo "bootstrap paths OK — Alpha and its assembler are owned by bootstrap/rungs/alpha"
