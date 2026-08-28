#!/usr/bin/env sh
set -eu

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$OMEGA_GATE_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_GATE_DIR/paths.sh"

fail() {
  echo "bootstrap paths: $*" >&2
  exit 1
}

expect_role() {
  role=$1
  expected=$2
  actual=$(bootstrap_path "$role") || fail "role rejected: $role"
  [ "$actual" = "$expected" ] || fail "$role resolved to $actual, expected $expected"
}

expect_role source "$OMEGA_REPO_ROOT/source"
expect_role alpha "$OMEGA_REPO_ROOT/source/alpha"
expect_role beta "$OMEGA_REPO_ROOT/source/beta"
expect_role gamma "$OMEGA_REPO_ROOT/source/gamma"
expect_role delta "$OMEGA_REPO_ROOT/source/delta"
expect_role delta-meaning "$OMEGA_REPO_ROOT/source/delta/meaning"
expect_role proof-kernel "$OMEGA_REPO_ROOT/source/proof-kernel"
expect_role omega-rust "$OMEGA_REPO_ROOT/source/omega-rust"
expect_role psi-rust "$OMEGA_REPO_ROOT/source/omega-rust/psi"
expect_role omega-rust-lowering "$OMEGA_REPO_ROOT/source/omega-rust/omega"
expect_role psi "$OMEGA_REPO_ROOT/source/psi"
expect_role omega "$OMEGA_REPO_ROOT/source/omega"

for required in \
  "$OMEGA_PATH_ALPHA" "$OMEGA_PATH_BETA" "$OMEGA_PATH_GAMMA" \
  "$OMEGA_PATH_DELTA" "$OMEGA_PATH_DELTA_MEANING" \
  "$OMEGA_PATH_PROOF_KERNEL" "$OMEGA_PATH_OMEGA_RUST" \
  "$OMEGA_PATH_PSI_PRODUCT" "$OMEGA_PATH_OMEGA_PRODUCT"
do
  [ -d "$required" ] || fail "required owner is absent: $required"
done

[ ! -e "$OMEGA_REPO_ROOT/source/on-ramp" ] || fail "retired source/on-ramp directory remains"
[ ! -e "$OMEGA_REPO_ROOT/source/refinement/delta-omega-bootstrap" ] || \
  fail "retired standalone bridge refinement remains"

echo "bootstrap paths: direct compiler-sequence owners verified"
