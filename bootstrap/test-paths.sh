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
[ "$OMEGA_PATH_PROOF_KERNEL_GATES" = "$OMEGA_PATH_PROOF_KERNEL/gates" ] ||
  fail "proof-kernel gate owner is $OMEGA_PATH_PROOF_KERNEL_GATES"
[ "$OMEGA_PATH_BETA_REFERENCE" = "$OMEGA_PATH_BETA/reference" ] ||
  fail "Beta reference owner is $OMEGA_PATH_BETA_REFERENCE"
[ "$OMEGA_PATH_BETA_REFINEMENT" = "$OMEGA_REPO_ROOT/bootstrap/assurance/refinement/beta" ] ||
  fail "Beta refinement owner is $OMEGA_PATH_BETA_REFINEMENT"
[ "$OMEGA_PATH_OMEGA0" = "$OMEGA_REPO_ROOT/bootstrap/omega0" ] ||
  fail "Omega0 owner is $OMEGA_PATH_OMEGA0"
[ "$OMEGA_PATH_CORPUS" = "$OMEGA_REPO_ROOT/bootstrap/corpus" ] ||
  fail "shared lattice corpus owner is $OMEGA_PATH_CORPUS"

[ -L "$OMEGA_REPO_ROOT/compiler/alpha" ] ||
  fail "compiler/alpha is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/beta" ] ||
  fail "compiler/beta is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/beta-lang" ] ||
  fail "compiler/beta-lang is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/proof-kernel" ] ||
  fail "compiler/proof-kernel is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/lattice-corpus" ] ||
  fail "compiler/lattice-corpus is not a temporary compatibility symlink"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/alpha")" = "$(physical_dir "$OMEGA_PATH_ALPHA")" ] ||
  fail "compiler/alpha does not resolve to the Alpha owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/beta")" = "$(physical_dir "$OMEGA_PATH_ALPHA_ASSEMBLER")" ] ||
  fail "compiler/beta does not resolve to the Alpha assembler owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/beta-lang")" = "$(physical_dir "$OMEGA_PATH_BETA")" ] ||
  fail "compiler/beta-lang does not resolve to the Beta owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/proof-kernel")" = "$(physical_dir "$OMEGA_PATH_PROOF_KERNEL")" ] ||
  fail "compiler/proof-kernel does not resolve to the proof-kernel owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/lattice-corpus")" = "$(physical_dir "$OMEGA_PATH_CORPUS")" ] ||
  fail "compiler/lattice-corpus does not resolve to the shared corpus owner"

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
[ "$(omega_bootstrap_path proof-kernel-gates)" = "$OMEGA_PATH_PROOF_KERNEL_GATES" ] ||
  fail "proof-kernel-gates role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path beta-reference)" = "$OMEGA_PATH_BETA_REFERENCE" ] ||
  fail "beta-reference role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path beta-refinement)" = "$OMEGA_PATH_BETA_REFINEMENT" ] ||
  fail "beta-refinement role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path omega0-gates)" = "$OMEGA_PATH_OMEGA0/gates" ] ||
  fail "omega0-gates role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path corpus)" = "$OMEGA_PATH_CORPUS" ] ||
  fail "corpus role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path lattice-corpus)" = "$OMEGA_PATH_CORPUS" ] ||
  fail "lattice-corpus compatibility role lookup disagrees with the manifest"

echo "bootstrap paths OK — rungs, assurance roles, Omega0, and shared corpus have canonical owners"
