#!/usr/bin/env sh
# Pin canonical bootstrap ownership and keep retired compiler aliases absent.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd -P)
. "$SCRIPT_DIR/paths.sh"

fail() {
  echo "bootstrap paths FAIL — $*" >&2
  exit 1
}

expect_role() {
  actual=$(omega_bootstrap_path "$1") || fail "canonical role is unavailable: $1"
  [ "$actual" = "$2" ] || fail "$1 resolves to $actual, expected $2"
}

reject_role() {
  if omega_bootstrap_path "$1" >/dev/null 2>&1; then
    fail "retired compatibility role is still accepted: $1"
  fi
}

[ "$OMEGA_PATH_ALPHA" = "$OMEGA_REPO_ROOT/source/alpha" ] ||
  fail "Alpha owner is $OMEGA_PATH_ALPHA"
[ "$OMEGA_PATH_ALPHA_ASSEMBLER" = "$OMEGA_PATH_ALPHA/assembler" ] ||
  fail "Alpha assembler owner is $OMEGA_PATH_ALPHA_ASSEMBLER"
[ "$OMEGA_PATH_BETA" = "$OMEGA_REPO_ROOT/source/beta" ] ||
  fail "Beta owner is $OMEGA_PATH_BETA"
[ "$OMEGA_PATH_BETA_REFERENCE" = "$OMEGA_PATH_BETA/reference" ] ||
  fail "Beta reference owner is $OMEGA_PATH_BETA_REFERENCE"
[ "$OMEGA_PATH_BETA_REFINEMENT" = "$OMEGA_REPO_ROOT/source/refinement/alpha-beta" ] ||
  fail "Beta refinement owner is $OMEGA_PATH_BETA_REFINEMENT"
[ "$OMEGA_PATH_GAMMA" = "$OMEGA_REPO_ROOT/source/gamma" ] ||
  fail "Gamma owner is $OMEGA_PATH_GAMMA"
[ "$OMEGA_PATH_DELTA" = "$OMEGA_REPO_ROOT/source/delta" ] ||
  fail "Delta owner is $OMEGA_PATH_DELTA"
[ "$OMEGA_PATH_PROOF_KERNEL" = "$OMEGA_REPO_ROOT/source/proof-kernel" ] ||
  fail "proof-kernel owner is $OMEGA_PATH_PROOF_KERNEL"
[ "$OMEGA_PATH_PROOF_KERNEL_GATES" = "$OMEGA_PATH_PROOF_KERNEL/gates" ] ||
  fail "proof-kernel gate owner is $OMEGA_PATH_PROOF_KERNEL_GATES"
[ "$OMEGA_PATH_REFINEMENT_ROOT" = "$OMEGA_REPO_ROOT/source/refinement" ] ||
  fail "refinement root is $OMEGA_PATH_REFINEMENT_ROOT"
[ "$OMEGA_PATH_OMEGA_BOOTSTRAP" = "$OMEGA_REPO_ROOT/source/on-ramp/omega-bootstrap" ] ||
  fail "omega-bootstrap owner is $OMEGA_PATH_OMEGA_BOOTSTRAP"
[ "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT" = "$OMEGA_PATH_REFINEMENT_ROOT/delta-omega-bootstrap" ] ||
  fail "omega-bootstrap refinement owner is $OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT"
[ "$OMEGA_PATH_CORPUS" = "$OMEGA_REPO_ROOT/tests/lattice/corpus" ] ||
  fail "shared corpus owner is $OMEGA_PATH_CORPUS"
[ "$OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT" = "$OMEGA_REPO_ROOT/source/on-ramp/rust" ] ||
  fail "Omega Rust on-ramp root is $OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT"
[ "$OMEGA_PATH_PSI_RUST" = "$OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT/psi" ] ||
  fail "Psi Rust on-ramp is $OMEGA_PATH_PSI_RUST"
[ "$OMEGA_PATH_OMEGA_RUST" = "$OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT/omega" ] ||
  fail "Omega Rust on-ramp is $OMEGA_PATH_OMEGA_RUST"
[ "$OMEGA_PATH_PSI_PRODUCT" = "$OMEGA_REPO_ROOT/source/psi" ] ||
  fail "Psi product owner is $OMEGA_PATH_PSI_PRODUCT"
[ "$OMEGA_PATH_OMEGA_PRODUCT" = "$OMEGA_REPO_ROOT/source/omega" ] ||
  fail "Omega product owner is $OMEGA_PATH_OMEGA_PRODUCT"

for owner in \
  "$OMEGA_PATH_ALPHA" "$OMEGA_PATH_ALPHA_ASSEMBLER" \
  "$OMEGA_PATH_BETA" "$OMEGA_PATH_BETA_REFERENCE" \
  "$OMEGA_PATH_BETA_REFINEMENT" "$OMEGA_PATH_GAMMA" \
  "$OMEGA_PATH_DELTA" \
  "$OMEGA_PATH_PROOF_KERNEL" "$OMEGA_PATH_OMEGA_BOOTSTRAP" \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT" "$OMEGA_PATH_CORPUS" \
  "$OMEGA_PATH_PSI_RUST" "$OMEGA_PATH_OMEGA_RUST" \
  "$OMEGA_PATH_PSI_PRODUCT" "$OMEGA_PATH_OMEGA_PRODUCT"; do
  [ -d "$owner" ] && [ ! -L "$owner" ] || fail "canonical owner is not a physical directory: $owner"
done

# The semantic-owner tree has no aggregate compiler or bootstrap directory.
for retired in bootstrap source/compiler source/assurance source/omega/source-checkpoints; do
  [ -z "$(git -C "$OMEGA_REPO_ROOT" ls-files "$retired" "$retired/*")" ] ||
    fail "retired tracked ownership bucket remains: $retired"
done

# Role-local links retain deliberately shared assurance/sample ownership. They
# are separate from the retired top-level compiler facade.
for entry in REFINEMENT.md alpha_refinement_check.py alpha_symbolic.py \
  refinement.sh refinement-cert-diamond.sh refinement-samples \
  refinement_compose_gen.py refinement_fork_gen.py refinement_fuzz_gen.py \
  refinement_loop_gen.py refinement_nested_gen.py; do
  [ -L "$OMEGA_PATH_ALPHA/$entry" ] || fail "Alpha refinement link is absent: $entry"
done
for entry in meaning-tv.sh input-tv.sh meaning-cert-diamond.sh \
  translation-validation.sh gamma2claim.py tv-encode.py meaning_cert_diamond.py; do
  [ -L "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/$entry" ] ||
    fail "omega-bootstrap refinement link is absent: $entry"
  [ -f "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/$entry" ] ||
    fail "omega-bootstrap refinement link is broken: $entry"
done
[ -L "$OMEGA_PATH_DELTA/samples/omega-bootstrap-frontend.alp" ] ||
  fail "canonical omega-bootstrap frontend sample link is absent"
[ -L "$OMEGA_PATH_DELTA/samples/omega0-frontend.alp" ] ||
  fail "historical Omega0 frontend sample link is absent"
[ -f "$OMEGA_PATH_DELTA/samples/omega-bootstrap-frontend.alp" ] ||
  fail "canonical omega-bootstrap frontend sample link is broken"
[ -f "$OMEGA_PATH_DELTA/samples/omega0-frontend.alp" ] ||
  fail "historical Omega0 frontend sample link is broken"

expect_role source "$OMEGA_PATH_SOURCE_ROOT"
expect_role on-ramp "$OMEGA_PATH_ON_RAMP_ROOT"
expect_role lattice-tests "$OMEGA_PATH_LATTICE_TEST_ROOT"
expect_role bootstrap-tools "$OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT"
expect_role alpha "$OMEGA_PATH_ALPHA"
expect_role alpha-assembler "$OMEGA_PATH_ALPHA_ASSEMBLER"
expect_role beta "$OMEGA_PATH_BETA"
expect_role beta-reference "$OMEGA_PATH_BETA_REFERENCE"
expect_role refinement "$OMEGA_PATH_REFINEMENT_ROOT"
expect_role beta-refinement "$OMEGA_PATH_BETA_REFINEMENT"
expect_role gamma "$OMEGA_PATH_GAMMA"
expect_role delta "$OMEGA_PATH_DELTA"
expect_role proof-kernel "$OMEGA_PATH_PROOF_KERNEL"
expect_role proof-kernel-gates "$OMEGA_PATH_PROOF_KERNEL_GATES"
expect_role proof-kernel-beta "$OMEGA_PATH_PROOF_KERNEL_BETA"
expect_role proof-kernel-reference "$OMEGA_PATH_PROOF_KERNEL_REFERENCE"
expect_role proof-kernel-gamma "$OMEGA_PATH_PROOF_KERNEL_GAMMA"
expect_role omega-bootstrap "$OMEGA_PATH_OMEGA_BOOTSTRAP"
expect_role omega-bootstrap-meaning "$OMEGA_PATH_OMEGA_BOOTSTRAP_MEANING"
expect_role omega-bootstrap-compiler "$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER"
expect_role omega-bootstrap-gates "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES"
expect_role omega-bootstrap-refinement "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT"
expect_role corpus "$OMEGA_PATH_CORPUS"
expect_role omega-rust-onramp "$OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT"
expect_role psi-rust "$OMEGA_PATH_PSI_RUST"
expect_role psi-rust/foundation "$OMEGA_PATH_PSI_RUST/foundation"
expect_role omega-rust "$OMEGA_PATH_OMEGA_RUST"
expect_role omega-rust/backend "$OMEGA_PATH_OMEGA_RUST/backend"
expect_role psi "$OMEGA_PATH_PSI_PRODUCT"
expect_role psi/foundation "$OMEGA_PATH_PSI_PRODUCT/foundation"
expect_role omega "$OMEGA_PATH_OMEGA_PRODUCT"
expect_role omega/backend "$OMEGA_PATH_OMEGA_PRODUCT/backend"

for role in alpha-assembler-rust beta-rust beta-rs beta-assembler beta-lang \
  beta-lang-rs beta-lang-py delta-rust delta-rs lattice-corpus source-checkpoints; do
  reject_role "$role"
done

# Omega0 aliases are a distinct migration surface and remain pinned until that
# terminology retirement is scheduled explicitly.
[ "$OMEGA_PATH_OMEGA0" = "$OMEGA_PATH_OMEGA_BOOTSTRAP" ] ||
  fail "Omega0 variable disagrees with omega-bootstrap"
[ "$OMEGA_PATH_OMEGA0_MEANING" = "$OMEGA_PATH_OMEGA_BOOTSTRAP_MEANING" ] ||
  fail "Omega0 meaning variable disagrees with omega-bootstrap"
[ "$OMEGA_PATH_OMEGA0_COMPILER" = "$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER" ] ||
  fail "Omega0 compiler variable disagrees with omega-bootstrap"
[ "$OMEGA_PATH_OMEGA0_GATES" = "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" ] ||
  fail "Omega0 gates variable disagrees with omega-bootstrap"
[ "$OMEGA_PATH_OMEGA0_REFINEMENT" = "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT" ] ||
  fail "Omega0 refinement variable disagrees with omega-bootstrap"
expect_role omega0 "$OMEGA_PATH_OMEGA_BOOTSTRAP"
expect_role omega0-meaning "$OMEGA_PATH_OMEGA_BOOTSTRAP_MEANING"
expect_role omega0-compiler "$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER"
expect_role omega0-gates "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES"
expect_role omega0-refinement "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT"
[ ! -e "$OMEGA_PATH_ON_RAMP_ROOT/omega0" ] || fail "obsolete omega0 on-ramp remains"

echo "bootstrap paths OK — semantic owners, on-ramps, refinements, and lattice tools resolve canonically"
