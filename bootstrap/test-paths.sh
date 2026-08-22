#!/usr/bin/env sh
# Pin canonical rung ownership paths and their temporary legacy aliases.
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
[ "$OMEGA_PATH_BETA" = "$OMEGA_REPO_ROOT/bootstrap/rungs/beta" ] ||
  fail "Beta owner is $OMEGA_PATH_BETA"
[ "$OMEGA_PATH_BETA_LANGUAGE" = "$OMEGA_PATH_BETA" ] ||
  fail "historical Beta language variable does not resolve to the Beta owner"
[ "$OMEGA_PATH_PROOF_KERNEL" = "$OMEGA_REPO_ROOT/bootstrap/assurance/proof-kernel" ] ||
  fail "proof-kernel owner is $OMEGA_PATH_PROOF_KERNEL"

[ -L "$OMEGA_REPO_ROOT/compiler/alpha" ] ||
  fail "compiler/alpha is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/beta" ] ||
  fail "compiler/beta is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/beta-lang" ] ||
  fail "compiler/beta-lang is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/proof-kernel" ] ||
  fail "compiler/proof-kernel is not a temporary compatibility symlink"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/alpha")" = "$(physical_dir "$OMEGA_PATH_ALPHA")" ] ||
  fail "compiler/alpha does not resolve to the Alpha owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/beta")" = "$(physical_dir "$OMEGA_PATH_ALPHA_ASSEMBLER")" ] ||
  fail "compiler/beta does not resolve to the Alpha assembler owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/beta-lang")" = "$(physical_dir "$OMEGA_PATH_BETA")" ] ||
  fail "compiler/beta-lang does not resolve to the Beta owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/proof-kernel")" = "$(physical_dir "$OMEGA_PATH_PROOF_KERNEL")" ] ||
  fail "compiler/proof-kernel does not resolve to the proof-kernel owner"

[ "$(omega_bootstrap_path alpha)" = "$OMEGA_PATH_ALPHA" ] ||
  fail "alpha role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path alpha-assembler)" = "$OMEGA_PATH_ALPHA_ASSEMBLER" ] ||
  fail "alpha-assembler role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path beta-assembler)" = "$OMEGA_PATH_ALPHA_ASSEMBLER" ] ||
  fail "beta-assembler compatibility role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path beta)" = "$OMEGA_PATH_BETA" ] ||
  fail "beta role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path beta-lang)" = "$OMEGA_PATH_BETA" ] ||
  fail "beta-lang compatibility role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path proof-kernel)" = "$OMEGA_PATH_PROOF_KERNEL" ] ||
  fail "proof-kernel role lookup disagrees with the manifest"

echo "bootstrap paths OK — language rungs and proof assurance have canonical owners"
