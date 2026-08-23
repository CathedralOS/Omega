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
[ "$OMEGA_PATH_GAMMA" = "$OMEGA_REPO_ROOT/bootstrap/rungs/gamma" ] ||
  fail "Gamma owner is $OMEGA_PATH_GAMMA"
[ "$OMEGA_PATH_PROOF_KERNEL" = "$OMEGA_REPO_ROOT/bootstrap/assurance/proof-kernel" ] ||
  fail "proof-kernel owner is $OMEGA_PATH_PROOF_KERNEL"
[ "$OMEGA_PATH_PROOF_KERNEL_GATES" = "$OMEGA_PATH_PROOF_KERNEL/gates" ] ||
  fail "proof-kernel gate owner is $OMEGA_PATH_PROOF_KERNEL_GATES"
[ "$OMEGA_PATH_BETA_REFERENCE" = "$OMEGA_PATH_BETA/reference" ] ||
  fail "Beta reference owner is $OMEGA_PATH_BETA_REFERENCE"
[ "$OMEGA_PATH_REFINEMENT_ROOT" = "$OMEGA_REPO_ROOT/bootstrap/assurance/refinement" ] ||
  fail "refinement root is $OMEGA_PATH_REFINEMENT_ROOT"
[ "$OMEGA_PATH_BETA_REFINEMENT" = "$OMEGA_REPO_ROOT/bootstrap/assurance/refinement/beta" ] ||
  fail "Beta refinement owner is $OMEGA_PATH_BETA_REFINEMENT"
[ "$OMEGA_PATH_OMEGA0_REFINEMENT" = "$OMEGA_REPO_ROOT/bootstrap/assurance/refinement/omega0" ] ||
  fail "Omega0 refinement owner is $OMEGA_PATH_OMEGA0_REFINEMENT"
[ "$OMEGA_PATH_ONRAMPS_ROOT" = "$OMEGA_REPO_ROOT/bootstrap/onramps" ] ||
  fail "on-ramp root is $OMEGA_PATH_ONRAMPS_ROOT"
[ "$OMEGA_PATH_DELTA" = "$OMEGA_REPO_ROOT/bootstrap/rungs/delta" ] ||
  fail "Delta owner is $OMEGA_PATH_DELTA"
[ "$OMEGA_PATH_DELTA_RUST" = "$OMEGA_REPO_ROOT/bootstrap/onramps/delta-rust" ] ||
  fail "Delta Rust on-ramp owner is $OMEGA_PATH_DELTA_RUST"
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
[ -L "$OMEGA_REPO_ROOT/compiler/gamma" ] ||
  fail "compiler/gamma is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/proof-kernel" ] ||
  fail "compiler/proof-kernel is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/lattice-corpus" ] ||
  fail "compiler/lattice-corpus is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/delta" ] ||
  fail "compiler/delta is not a temporary compatibility symlink"
[ -L "$OMEGA_REPO_ROOT/compiler/delta-rs" ] ||
  fail "compiler/delta-rs is not a temporary compatibility symlink"
[ -L "$OMEGA_PATH_DELTA_RUST/samples" ] ||
  fail "Delta Rust samples is not a compatibility symlink"
for entry in REFINEMENT.md alpha_refinement_check.py alpha_symbolic.py \
  refinement.sh refinement-cert-diamond.sh refinement-samples \
  refinement_compose_gen.py refinement_fork_gen.py refinement_fuzz_gen.py \
  refinement_loop_gen.py refinement_nested_gen.py; do
  [ -L "$OMEGA_PATH_ALPHA/$entry" ] ||
    fail "Alpha refinement compatibility entry is not a symlink: $entry"
done
for entry in meaning-tv.sh input-tv.sh meaning-cert-diamond.sh \
  translation-validation.sh gamma2claim.py tv-encode.py \
  meaning_cert_diamond.py; do
  [ -L "$OMEGA_PATH_OMEGA0_GATES/$entry" ] ||
    fail "Omega0 refinement compatibility entry is not a symlink: $entry"
done
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/alpha")" = "$(physical_dir "$OMEGA_PATH_ALPHA")" ] ||
  fail "compiler/alpha does not resolve to the Alpha owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/beta")" = "$(physical_dir "$OMEGA_PATH_ALPHA_ASSEMBLER")" ] ||
  fail "compiler/beta does not resolve to the Alpha assembler owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/beta-lang")" = "$(physical_dir "$OMEGA_PATH_BETA")" ] ||
  fail "compiler/beta-lang does not resolve to the Beta owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/gamma")" = "$(physical_dir "$OMEGA_PATH_GAMMA")" ] ||
  fail "compiler/gamma does not resolve to the Gamma owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/proof-kernel")" = "$(physical_dir "$OMEGA_PATH_PROOF_KERNEL")" ] ||
  fail "compiler/proof-kernel does not resolve to the proof-kernel owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/lattice-corpus")" = "$(physical_dir "$OMEGA_PATH_CORPUS")" ] ||
  fail "compiler/lattice-corpus does not resolve to the shared corpus owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/delta")" = "$(physical_dir "$OMEGA_PATH_DELTA")" ] ||
  fail "compiler/delta does not resolve to the Delta owner"
[ "$(physical_dir "$OMEGA_REPO_ROOT/compiler/delta-rs")" = "$(physical_dir "$OMEGA_PATH_DELTA_RUST")" ] ||
  fail "compiler/delta-rs does not resolve to the Delta Rust on-ramp"
[ "$(physical_dir "$OMEGA_PATH_DELTA_RUST/samples")" = "$(physical_dir "$OMEGA_PATH_DELTA/samples")" ] ||
  fail "Delta Rust samples does not resolve to the Delta rung corpus"

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
[ "$(omega_bootstrap_path gamma)" = "$OMEGA_PATH_GAMMA" ] ||
  fail "gamma role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path proof-kernel)" = "$OMEGA_PATH_PROOF_KERNEL" ] ||
  fail "proof-kernel role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path proof-kernel-gates)" = "$OMEGA_PATH_PROOF_KERNEL_GATES" ] ||
  fail "proof-kernel-gates role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path beta-reference)" = "$OMEGA_PATH_BETA_REFERENCE" ] ||
  fail "beta-reference role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path refinement)" = "$OMEGA_PATH_REFINEMENT_ROOT" ] ||
  fail "refinement role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path beta-refinement)" = "$OMEGA_PATH_BETA_REFINEMENT" ] ||
  fail "beta-refinement role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path omega0-refinement)" = "$OMEGA_PATH_OMEGA0_REFINEMENT" ] ||
  fail "omega0-refinement role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path delta)" = "$OMEGA_PATH_DELTA" ] ||
  fail "delta role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path delta-rs)" = "$OMEGA_PATH_DELTA_RUST" ] ||
  fail "delta-rs role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path omega0-gates)" = "$OMEGA_PATH_OMEGA0/gates" ] ||
  fail "omega0-gates role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path corpus)" = "$OMEGA_PATH_CORPUS" ] ||
  fail "corpus role lookup disagrees with the manifest"
[ "$(omega_bootstrap_path lattice-corpus)" = "$OMEGA_PATH_CORPUS" ] ||
  fail "lattice-corpus compatibility role lookup disagrees with the manifest"

echo "bootstrap paths OK — rungs, on-ramps, assurance roles, Omega0, and shared corpus have canonical owners"
