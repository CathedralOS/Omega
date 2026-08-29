#!/usr/bin/env sh
# Reject executable lattice gates that reach across ownership roots with
# sibling-relative paths. Local paths remain legitimate;
# cross-owner paths must come from tools/lattice/paths.sh.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd -P)
. "$SCRIPT_DIR/paths.sh"

# Pin canonical role ownership and the absence of the retired compiler facade,
# then prevent new topology-dependent cross-owner paths.
sh "$SCRIPT_DIR/test-paths.sh"

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

echo "lattice path hygiene OK"
