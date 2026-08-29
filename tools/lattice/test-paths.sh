#!/usr/bin/env sh
set -eu

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$OMEGA_GATE_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_GATE_DIR/paths.sh"

OMEGA_LATTICE_RUNNER="$OMEGA_GATE_DIR/verify-lattice.sh"

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

expect_rejected_role() {
  if lattice_path "$1" >/dev/null 2>&1; then
    fail "non-chain role was accepted: $1"
  fi
}

expect_role alpha "$OMEGA_REPO_ROOT/source/alpha"
expect_role beta-compiler "$OMEGA_REPO_ROOT/source/beta/compiler"
expect_role gamma-compiler "$OMEGA_REPO_ROOT/source/gamma/compiler"
expect_role delta-compiler "$OMEGA_REPO_ROOT/source/delta/compiler"
expect_role omega "$OMEGA_REPO_ROOT/source/omega"

for non_chain_role in alpha-assembler alpha-checker beta-validation beta-reference \
  gamma delta psi lattice-tools
do
  expect_rejected_role "$non_chain_role"
done

for required in \
  "$OMEGA_PATH_ALPHA" "$OMEGA_PATH_ALPHA_ASSEMBLER" \
  "$OMEGA_PATH_ALPHA_CHECKER" "$OMEGA_PATH_BETA" "$OMEGA_PATH_GAMMA" \
  "$OMEGA_PATH_GAMMA_COMPILER" "$OMEGA_PATH_DELTA" "$OMEGA_PATH_DELTA_COMPILER" \
  "$OMEGA_PATH_OMEGA" \
  "$OMEGA_PATH_BETA_COMPILER" "$OMEGA_PATH_BETA_VALIDATION"
do
  [ -d "$required" ] || fail "required owner is absent: $required"
done


[ "$OMEGA_PATH_BETA_COMPILER_SOURCE" = "$OMEGA_PATH_BETA_COMPILER/beta_compiler.alpha" ] ||
  fail "Beta compiler source locator is not canonical"
[ "$OMEGA_PATH_BETA_COMPILER_TAPE" = "$OMEGA_PATH_BETA_COMPILER/beta_compiler_bytecode.tape" ] ||
  fail "Beta compiler tape locator is not canonical"
[ "$OMEGA_PATH_GAMMA_COMPILER_SOURCE" = "$OMEGA_PATH_GAMMA_COMPILER/gamma_compiler.beta" ] ||
  fail "Gamma compiler source locator is not canonical"
[ "$OMEGA_PATH_GAMMA_COMPILER_TAPE" = "$OMEGA_PATH_GAMMA_COMPILER/gamma_compiler_bytecode.tape" ] ||
  fail "Gamma compiler tape locator is not canonical"
[ "$OMEGA_PATH_DELTA_COMPILER_SOURCE" = "$OMEGA_PATH_DELTA_COMPILER/delta_compiler.gamma" ] ||
  fail "Delta compiler source locator is not canonical"
[ "$OMEGA_PATH_DELTA_COMPILER_TAPE" = "$OMEGA_PATH_DELTA_COMPILER/delta_compiler_bytecode.tape" ] ||
  fail "Delta compiler tape locator is not canonical"
[ "$OMEGA_PATH_OMEGA_D_SOURCE" = "$OMEGA_PATH_OMEGA/omega_compiler.delta" ] ||
  fail "Omega D source locator is not canonical"
[ "$OMEGA_PATH_OMEGA0_COMPILER_TAPE" = "$OMEGA_PATH_OMEGA/omega0_compiler_bytecode.tape" ] ||
  fail "omega0 tape locator is not canonical"
[ "$OMEGA_PATH_OMEGA_C_BUILD" = "$OMEGA_PATH_OMEGA/build.omg" ] ||
  fail "Omega C build locator is not canonical"
[ "$OMEGA_PATH_OMEGA_C_MAIN" = "$OMEGA_PATH_OMEGA/main.omg" ] ||
  fail "Omega C main locator is not canonical"
[ "$OMEGA_PATH_OMEGA_COMPILER_TAPE" = "$OMEGA_PATH_OMEGA/omega_compiler_bytecode.tape" ] ||
  fail "Omega compiler tape locator is not canonical"

[ -f "$OMEGA_PATH_BETA_COMPILER_SOURCE" ] || fail "canonical Beta compiler source is absent"
[ -f "$OMEGA_PATH_BETA_COMPILER_TAPE" ] || fail "canonical Beta compiler tape is absent"
[ -f "$OMEGA_PATH_OMEGA_C_BUILD" ] || fail "canonical Omega C build root is absent"
[ -f "$OMEGA_PATH_OMEGA_C_MAIN" ] || fail "canonical Omega C main root is absent"

# The source root may host product/reference owners beside the lattice, but no
# unclassified top-level owner may silently become another bootstrap route.
tracked_source_roots=$(git -C "$OMEGA_REPO_ROOT" ls-files source | \
  awk -F/ 'NF > 2 { print $2 }' | sort -u)
expected_source_roots='alpha
beta
delta
gamma
library
omega
omega-rust
psi'
[ "$tracked_source_roots" = "$expected_source_roots" ] ||
  fail "tracked source owners differ from the classified source-root set"

# Canonical compiler-shaped source and tape names are a positive allow-list.
# Future entries may be absent while their language contract is open, but no
# alternate spelling, suffix, nested tape, or native compiler identity may
# appear in their place.
tracked_compiler_sources=$(git -C "$OMEGA_REPO_ROOT" ls-files \
  source/beta source/gamma source/delta source/omega | \
  grep -E '/[^/]*compiler\.(alpha|beta|gamma|delta|omg)$' || true)
expected_compiler_sources='source/beta/compiler/beta_compiler.alpha'
[ "$tracked_compiler_sources" = "$expected_compiler_sources" ] ||
  fail "compiler source exists outside the canonical implemented location"

tracked_compiler_tapes=$(git -C "$OMEGA_REPO_ROOT" ls-files \
  source/beta source/gamma source/delta source/omega | \
  grep -E '/[^/]*compiler[^/]*\.tape$' || true)
expected_compiler_tapes='source/beta/compiler/beta_compiler_bytecode.tape'
[ "$tracked_compiler_tapes" = "$expected_compiler_tapes" ] ||
  fail "compiler tape exists outside the canonical implemented location"

tracked_native_compilers=$(git -C "$OMEGA_REPO_ROOT" ls-files \
  source/beta source/gamma source/delta source/omega | \
  grep -E '\.(exe|elf|dll|dylib|so|a|o|obj|wasm)$' || true)
[ -z "$tracked_native_compilers" ] ||
  fail "native compiler artifact exists above the Alpha seed: $tracked_native_compilers"

for canonical_owner in \
  "$OMEGA_PATH_ALPHA" "$OMEGA_PATH_BETA" "$OMEGA_PATH_GAMMA" \
  "$OMEGA_PATH_DELTA" "$OMEGA_PATH_OMEGA"
do
  generic_buckets=$(find "$canonical_owner" -type d \
    \( -name bootstrap -o -name on-ramp -o -name assurance -o -name canaries \) \
    -print)
  [ -z "$generic_buckets" ] ||
    fail "generic ownership bucket remains under $canonical_owner: $generic_buckets"
done

# Retained infrastructure below the audited floor must justify every tracked
# file and child directory next to its owner. A child without its own README is
# a leaf inventory and must name every file in the parent's retention table.
# This prevents an approved bucket from becoming an exemption where unrelated
# files can silently accumulate.
require_retention_inventory() { # repository-relative owner directory
  inventory_owner=$1
  inventory_readme="$OMEGA_REPO_ROOT/$inventory_owner/README.md"
  [ -f "$inventory_readme" ] ||
    fail "retained owner lacks adjacent README: $inventory_owner"
  grep -Fq 'Deletion condition' "$inventory_readme" ||
    fail "retained owner lacks deletion conditions: $inventory_owner"

  inventory_depth=$(printf '%s' "$inventory_owner" | awk -F/ '{ print NF + 1 }')
  inventory_files=$(git -C "$OMEGA_REPO_ROOT" ls-files "$inventory_owner" | \
    awk -F/ -v depth="$inventory_depth" 'NF == depth { print $NF }')
  for inventory_file in $inventory_files; do
    [ "$inventory_file" = README.md ] && continue
    [ -e "$OMEGA_REPO_ROOT/$inventory_owner/$inventory_file" ] || continue
    grep -Fq "\`$inventory_file\`" "$inventory_readme" ||
      fail "tracked file lacks retention row: $inventory_owner/$inventory_file"
  done

  inventory_children=$(git -C "$OMEGA_REPO_ROOT" ls-files "$inventory_owner" | \
    awk -F/ -v depth="$inventory_depth" \
      'NF >= depth { print $depth }' | sort -u)
  for inventory_child in $inventory_children; do
    [ -d "$OMEGA_REPO_ROOT/$inventory_owner/$inventory_child" ] || continue
    grep -Fq "\`$inventory_child/\`" "$inventory_readme" ||
      fail "tracked child lacks retention row: $inventory_owner/$inventory_child"
    [ -f "$OMEGA_REPO_ROOT/$inventory_owner/$inventory_child/README.md" ] && continue
    inventory_leaf_files=$(git -C "$OMEGA_REPO_ROOT" \
      ls-files "$inventory_owner/$inventory_child" | awk -F/ '{ print $NF }')
    for inventory_leaf_file in $inventory_leaf_files; do
      [ -e "$OMEGA_REPO_ROOT/$inventory_owner/$inventory_child/$inventory_leaf_file" ] || continue
      grep -Fq "\`$inventory_leaf_file\`" "$inventory_readme" ||
        fail "leaf file lacks retention row: $inventory_owner/$inventory_child/$inventory_leaf_file"
    done
  done
}

for inventoried_owner in \
  source/alpha \
  source/alpha/assembler \
  source/alpha/checker \
  source/alpha/checker/artifacts \
  source/alpha/checker/corpus \
  source/alpha/checker/gates \
  source/alpha/checker/implementations \
  source/beta \
  source/beta/compiler \
  source/beta/compiler/validation \
  source/beta/compiler/validation/admission \
  source/beta/reference \
  source/gamma \
  source/gamma/compiler \
  source/gamma/reference \
  source/delta \
  source/delta/compiler \
  source/omega
do
  require_retention_inventory "$inventoried_owner"
done

[ ! -e "$OMEGA_REPO_ROOT/source/on-ramp" ] || fail "retired source/on-ramp directory remains"
[ ! -e "$OMEGA_REPO_ROOT/source/proof-kernel" ] || fail "orphan proof-kernel owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/refinement" ] || fail "generic refinement owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/assurance" ] || fail "generic assurance owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/bootstrap" ] || fail "generic source bootstrap owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/canaries" ] || fail "generic source canaries owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/omega-bootstrap" ] || fail "standalone omega-bootstrap owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/omega0" ] || fail "omega0 output incorrectly owns source"
[ ! -e "$OMEGA_REPO_ROOT/source/omega1" ] || fail "omega1 output incorrectly owns source"
[ ! -e "$OMEGA_REPO_ROOT/source/omega-boot" ] || fail "standalone omega-boot owner remains"
[ ! -e "$OMEGA_PATH_OMEGA/psi" ] || fail "Psi remains nested under the Omega product owner"
[ ! -e "$OMEGA_PATH_OMEGA/bootstrap" ] || fail "Omega product source contains a bootstrap owner"
[ ! -e "$OMEGA_REPO_ROOT/source/delta/build" ] || fail "unowned Delta build bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/source/delta/meaning" ] || fail "retired Delta-to-Gamma meaning owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/delta/compiler/validation" ] || fail "retired Delta native-publication validation remains"
[ ! -e "$OMEGA_REPO_ROOT/source/gamma/compatibility" ] || fail "retired Gamma compatibility bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/source/gamma/canonical-bytes" ] || fail "unowned Gamma canonical-byte bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/source/gamma/terminal-codec-primitives" ] || fail "unowned Gamma terminal-codec bucket remains"
[ ! -e "$OMEGA_PATH_DELTA/source-closures" ] || fail "Delta compiler validation records remain at the language root"
[ ! -e "$OMEGA_PATH_BETA_COMPILER/artifacts" ] || fail "Beta compiler artifact remains in a nested artifacts bucket"
[ ! -e "$OMEGA_PATH_BETA_VALIDATION/stress" ] || fail "generic Beta stress bucket remains"
[ ! -e "$OMEGA_PATH_BETA_VALIDATION/admission/fol" ] || fail "toy Beta FOL capability seam remains"
[ ! -e "$OMEGA_PATH_GAMMA_COMPILER/artifacts" ] || fail "Gamma compiler artifact remains in a nested artifacts bucket"
[ ! -e "$OMEGA_PATH_DELTA_COMPILER/artifacts" ] || fail "Delta compiler artifact remains in a nested artifacts bucket"
[ ! -e "$OMEGA_PATH_OMEGA/artifacts" ] || fail "Omega compiler artifacts remain in a nested artifacts bucket"
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

# The only closed chain row is Alpha. The checker is a service beside the
# chain, and no differently labelled open edge may enter through a blacklist
# gap, so pin the complete step set rather than selected forbidden labels.
expected_steps='step "alpha — seed behavior and exact assembler construction" alpha verify.sh --edge'
actual_steps=$(grep '^step "' "$OMEGA_LATTICE_RUNNER" || true)
[ "$actual_steps" = "$expected_steps" ] ||
  fail "default lattice step set is not exactly the closed Alpha floor"

for diagnostic in \
  'check-path-hygiene.sh' \
  'selfhost.sh' \
  'reconstruct-artifact.sh' \
  'admission/bc-artifact-structure.sh' \
  'test-interp.sh' \
  'test-typeck.sh' \
  'compiler/rebuild-artifact.sh' \
  'source-closure-snapshot-v1.sh' \
  'lower-rooted-assembly-publication-v1-test.sh'
do
  if grep -Fq "$diagnostic" "$OMEGA_LATTICE_RUNNER"; then
    fail "diagnostic-only command returned to the default lattice: $diagnostic"
  fi
done

echo "lattice paths: canonical owners and retired-route absence verified"
