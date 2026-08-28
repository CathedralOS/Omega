#!/usr/bin/env sh
set -eu

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$OMEGA_GATE_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_GATE_DIR/paths.sh"

fail() {
  echo "lattice paths: $*" >&2
  exit 1
}

expect_role() {
  role=$1
  expected=$2
  actual=$(lattice_path "$role") || fail "role rejected: $role"
  [ "$actual" = "$expected" ] || fail "$role resolved to $actual, expected $expected"
}

expect_role alpha "$OMEGA_REPO_ROOT/source/alpha"
expect_role alpha-checker "$OMEGA_REPO_ROOT/source/alpha/checker"
expect_role beta-compiler "$OMEGA_REPO_ROOT/source/beta/compiler"
expect_role beta-validation "$OMEGA_REPO_ROOT/source/beta/compiler/validation"
expect_role gamma "$OMEGA_REPO_ROOT/source/gamma"
expect_role delta "$OMEGA_REPO_ROOT/source/delta"
expect_role delta-compiler "$OMEGA_REPO_ROOT/source/delta/compiler"
expect_role delta-validation "$OMEGA_REPO_ROOT/source/delta/compiler/validation"
expect_role omega "$OMEGA_REPO_ROOT/source/omega"
expect_role omega-psi "$OMEGA_REPO_ROOT/source/omega/psi"
expect_role lattice-tools "$OMEGA_REPO_ROOT/tools/lattice"

for required in \
  "$OMEGA_PATH_ALPHA" "$OMEGA_PATH_BETA" "$OMEGA_PATH_GAMMA" \
  "$OMEGA_PATH_DELTA" "$OMEGA_PATH_DELTA_COMPILER" \
  "$OMEGA_PATH_DELTA_VALIDATION" "$OMEGA_PATH_DELTA_MEANING" \
  "$OMEGA_PATH_OMEGA" "$OMEGA_PATH_OMEGA_PSI" \
  "$OMEGA_PATH_BETA_COMPILER" "$OMEGA_PATH_BETA_VALIDATION" \
  "$OMEGA_PATH_ALPHA_CHECKER"
do
  [ -d "$required" ] || fail "required owner is absent: $required"
done

[ ! -e "$OMEGA_REPO_ROOT/source/on-ramp" ] || fail "retired source/on-ramp directory remains"
[ ! -e "$OMEGA_REPO_ROOT/source/psi" ] || fail "orphan product Psi owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/proof-kernel" ] || fail "orphan proof-kernel owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/refinement" ] || fail "generic refinement owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/assurance" ] || fail "generic assurance owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/bootstrap" ] || fail "generic source bootstrap owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/canaries" ] || fail "generic source canaries owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/omega-bootstrap" ] || fail "standalone omega-bootstrap owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/delta/build" ] || fail "unowned Delta build bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/source/gamma/compatibility" ] || fail "retired Gamma compatibility bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/source/gamma/canonical-bytes" ] || fail "unowned Gamma canonical-byte bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/source/gamma/terminal-codec-primitives" ] || fail "unowned Gamma terminal-codec bucket remains"
[ ! -e "$OMEGA_PATH_DELTA/source-closures" ] || fail "Delta compiler validation records remain at the language root"
for misplaced in \
  "$OMEGA_PATH_DELTA"/DELTA_*_V1.md \
  "$OMEGA_PATH_DELTA"/lower-rooted-* \
  "$OMEGA_PATH_DELTA"/lower_rooted_* \
  "$OMEGA_PATH_DELTA"/publication-support-* \
  "$OMEGA_PATH_DELTA"/publication_support* \
  "$OMEGA_PATH_DELTA"/source-closure-* \
  "$OMEGA_PATH_DELTA"/source_closure_*
do
  [ ! -e "$misplaced" ] || fail "Delta compiler validation file remains at the language root: $misplaced"
done
[ ! -e "$OMEGA_REPO_ROOT/bootstrap" ] || fail "generic bootstrap bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/canaries" ] || fail "generic canaries bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/tests/bootstrap" ] || fail "generic bootstrap test bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/tests/canaries" ] || fail "generic canaries test bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/tools/bootstrap" ] || fail "generic bootstrap tooling bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/tools/assurance" ] || fail "generic assurance tooling bucket remains"

echo "lattice paths: direct compiler-sequence owners verified"
