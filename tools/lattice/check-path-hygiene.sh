#!/usr/bin/env sh
# Enforce the direct compiler-lattice topology and reject executable gates
# that reach across ownership roots. This is the single owned topology gate.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd -P)
. "$SCRIPT_DIR/paths.sh"

# Pin canonical role ownership and the absence of the retired compiler facade,
# then prevent new topology-dependent cross-owner paths.


fail() {
  echo "lattice paths: $*" >&2
  exit 1
}

for required in \
  "$OMEGA_PATH_ALPHA" "$OMEGA_PATH_ALPHA_ASSEMBLER" \
  "$OMEGA_PATH_ALPHA_CHECKER" "$OMEGA_PATH_BETA" "$OMEGA_PATH_GAMMA" \
  "$OMEGA_PATH_GAMMA_COMPILER" "$OMEGA_PATH_DELTA" "$OMEGA_PATH_DELTA_COMPILER" \
  "$OMEGA_PATH_OMEGA" \
  "$OMEGA_PATH_BETA_COMPILER" "$OMEGA_PATH_BETA_COMPILER/validation"
do
  [ -d "$required" ] || fail "required owner is absent: $required"
done


[ "$OMEGA_PATH_BETA_COMPILER_SOURCE" = "$OMEGA_PATH_BETA_COMPILER/beta_compiler.alpha" ] ||
  fail "Beta compiler source locator is not canonical"
[ "$OMEGA_PATH_BETA_COMPILER_TAPE" = "$OMEGA_PATH_BETA_COMPILER/beta_compiler_bytecode.tape" ] ||
  fail "Beta compiler tape locator is not canonical"
[ "$OMEGA_PATH_OMEGA_COMPILER_SOURCE" = "$OMEGA_PATH_OMEGA/omega_compiler.delta" ] ||
  fail "Omega D compiler source locator is not canonical"

[ -f "$OMEGA_PATH_BETA_COMPILER_SOURCE" ] || fail "canonical Beta compiler source is absent"
[ -f "$OMEGA_PATH_BETA_COMPILER_TAPE" ] || fail "canonical Beta compiler tape is absent"
[ -f "$OMEGA_PATH_GAMMA_COMPILER/gamma_compiler.beta" ] ||
  fail "canonical Gamma compiler source is absent"
[ -f "$OMEGA_PATH_OMEGA_COMPILER_SOURCE" ] ||
  fail "canonical Omega D compiler source is absent"
[ -f "$OMEGA_PATH_OMEGA/build.omg" ] || fail "canonical Omega C build root is absent"
[ -f "$OMEGA_PATH_OMEGA/main.omg" ] || fail "canonical Omega C main root is absent"

# D15 applies to the exact currently retained Alpha-through-Delta
# implementation-source membership, not to every file sharing a suffix. Keep
# this list explicit as compiler closures are added or retired. Each byte must
# be HT, LF, CR, or printable ASCII before any language tokenizer sees it.
for bootstrap_ascii_source in \
  source/alpha/assembler/assembler.alpha \
  source/alpha/assembler/examples/echo.alpha \
  source/alpha/assembler/examples/factorial.alpha \
  source/alpha/assembler/examples/fib.alpha \
  source/alpha/assembler/examples/gcd.alpha \
  source/alpha/assembler/examples/multiply.alpha \
  source/alpha/checker/implementations/beta/check.beta \
  source/alpha/checker/implementations/beta/eq.beta \
  source/beta/compiler/beta_compiler.alpha \
  source/beta/compiler/validation/admission/bc-artifact-structure.alpha \
  source/delta/compiler/delta_compiler.gamma \
  source/gamma/interp.beta \
  source/gamma/compiler/gamma_compiler.beta \
  source/omega/omega_compiler.delta
do
  bootstrap_ascii_path="$OMEGA_REPO_ROOT/$bootstrap_ascii_source"
  [ -f "$bootstrap_ascii_path" ] ||
    fail "D15 source member is absent: $bootstrap_ascii_source"
  if ! od -An -tu1 -v "$bootstrap_ascii_path" | awk '
    {
      for (i = 1; i <= NF; i++) {
        b = $i + 0
        if (b != 9 && b != 10 && b != 13 && (b < 32 || b > 126)) exit 1
      }
    }
  '; then
    fail "D15 source member contains a forbidden byte: $bootstrap_ascii_source"
  fi
done
unset bootstrap_ascii_source bootstrap_ascii_path

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
expected_compiler_sources='source/beta/compiler/beta_compiler.alpha
source/delta/compiler/delta_compiler.gamma
source/gamma/compiler/gamma_compiler.beta
source/omega/omega_compiler.delta'
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
[ ! -e "$OMEGA_REPO_ROOT/source/psi/generated" ] || fail "Psi semantic source remains under a generic generated owner"
[ ! -e "$OMEGA_REPO_ROOT/source/delta/build" ] || fail "unowned Delta build bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/source/delta/meaning" ] || fail "retired Delta-to-Gamma meaning owner remains"
[ ! -e "$OMEGA_REPO_ROOT/source/delta/compiler/validation" ] || fail "retired Delta native-publication validation remains"
[ ! -e "$OMEGA_REPO_ROOT/source/gamma/compatibility" ] || fail "retired Gamma compatibility bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/source/gamma/canonical-bytes" ] || fail "unowned Gamma canonical-byte bucket remains"
[ ! -e "$OMEGA_REPO_ROOT/source/gamma/terminal-codec-primitives" ] || fail "unowned Gamma terminal-codec bucket remains"
[ ! -e "$OMEGA_PATH_DELTA/source-closures" ] || fail "Delta compiler validation records remain at the language root"
[ ! -e "$OMEGA_PATH_BETA_COMPILER/artifacts" ] || fail "Beta compiler artifact remains in a nested artifacts bucket"
[ ! -e "$OMEGA_PATH_BETA_COMPILER/validation/stress" ] || fail "generic Beta stress bucket remains"
[ ! -e "$OMEGA_PATH_BETA_COMPILER/validation/admission/fol" ] || fail "toy Beta FOL capability seam remains"
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


pattern='\.\./(alpha|beta|beta-rs|beta-lang|beta-lang-rs|beta-lang-py|gamma|delta|delta-rs|proof-kernel|omega|lattice-corpus|psi)(/|[^A-Za-z0-9_-]|$)'

if command -v rg >/dev/null 2>&1; then
  violations=$(rg -n --glob '*.sh' --glob '*.py' \
    --glob '!**/target/**' --glob '!**/build/**' "$pattern" \
    "$OMEGA_REPO_ROOT/source" "$OMEGA_REPO_ROOT/tools/lattice" || true)
else
  violations=$(find "$OMEGA_REPO_ROOT/source" "$OMEGA_REPO_ROOT/tools/lattice" \
    \( -path '*/target' -o -path '*/build' \) -prune -o \
    -type f \( -name '*.sh' -o -name '*.py' \) \
    -exec grep -En "$pattern" {} + || true)
fi

if [ -n "$violations" ]; then
  echo "lattice path hygiene FAIL — cross-owner sibling paths remain:" >&2
  printf '%s\n' "$violations" >&2
  exit 1
fi

# A compiler owner may coordinate with its immediate predecessor and consume
# the exact immediate-successor source subject. It may not import source or a
# semantic executable from any later owner through an exported absolute path.
check_forward_boundary() { # owner label forbidden-pattern
  boundary_owner=$1
  boundary_label=$2
  boundary_pattern=$3
  if command -v rg >/dev/null 2>&1; then
    boundary_violations=$(rg -n \
      --glob '*.sh' --glob '*.py' --glob '*.alpha' --glob '*.beta' \
      --glob '*.gamma' --glob '*.delta' --glob '*.omg' \
      "$boundary_pattern" "$boundary_owner" || true)
  else
    boundary_violations=$(find "$boundary_owner" -type f \
      \( -name '*.sh' -o -name '*.py' -o -name '*.alpha' -o \
         -name '*.beta' -o -name '*.gamma' -o -name '*.delta' -o \
         -name '*.omg' \) -exec grep -En "$boundary_pattern" {} + || true)
  fi
  if [ -n "$boundary_violations" ]; then
    echo "lattice path hygiene FAIL — $boundary_label reaches beyond its immediate successor:" >&2
    printf '%s\n' "$boundary_violations" >&2
    exit 1
  fi
}

check_forward_boundary "$OMEGA_PATH_BETA_COMPILER" "Beta compiler owner" \
  'OMEGA_PATH_(DELTA|OMEGA)|OMEGA_REPO_ROOT.*/source/(delta|omega|psi)(/|[^A-Za-z0-9_-]|$)|source/(delta|omega|psi)(/|[^A-Za-z0-9_-]|$)'
check_forward_boundary "$OMEGA_PATH_GAMMA_COMPILER" "Gamma compiler owner" \
  'OMEGA_PATH_OMEGA|OMEGA_REPO_ROOT.*/source/(omega|psi)(/|[^A-Za-z0-9_-]|$)|source/(omega|psi)(/|[^A-Za-z0-9_-]|$)'
check_forward_boundary "$OMEGA_PATH_DELTA_COMPILER" "Delta compiler owner" \
  'OMEGA_PATH_PSI|OMEGA_REPO_ROOT.*/source/psi(/|[^A-Za-z0-9_-]|$)|source/psi(/|[^A-Za-z0-9_-]|$)'

# The retired external Delta producer must not re-enter source or lattice
# tooling under either its old role variable or a new Rust subtree.
retired_delta_pattern='OMEGA_PATH_DELTA''_RUST|source/omega-rust/''delta'
if command -v rg >/dev/null 2>&1; then
  retired_delta_violations=$(rg -n "$retired_delta_pattern" \
    "$OMEGA_REPO_ROOT/source" \
    "$OMEGA_REPO_ROOT/tools/lattice" || true)
else
  retired_delta_violations=$(grep -R -En "$retired_delta_pattern" \
    "$OMEGA_REPO_ROOT/source" \
    "$OMEGA_REPO_ROOT/tools/lattice" || true)
fi

if [ -n "$retired_delta_violations" ]; then
  echo "lattice path hygiene FAIL — retired external Delta producer remains:" >&2
  printf '%s\n' "$retired_delta_violations" >&2
  exit 1
fi

# The universal checker is Alpha-owned. Keep the path role named by that owner
# instead of recreating a repository-level proof-kernel abstraction.
retired_checker_pattern='OMEGA_PATH_PROOF''_KERNEL'
if command -v rg >/dev/null 2>&1; then
  retired_checker_violations=$(rg -n "$retired_checker_pattern" \
    "$OMEGA_REPO_ROOT/source" \
    "$OMEGA_REPO_ROOT/tools/lattice" || true)
else
  retired_checker_violations=$(grep -R -En "$retired_checker_pattern" \
    "$OMEGA_REPO_ROOT/source" \
    "$OMEGA_REPO_ROOT/tools/lattice" || true)
fi

if [ -n "$retired_checker_violations" ]; then
  echo "lattice path hygiene FAIL — checker path is not Alpha-owned:" >&2
  printf '%s\n' "$retired_checker_violations" >&2
  exit 1
fi

echo "lattice topology and path hygiene OK"
