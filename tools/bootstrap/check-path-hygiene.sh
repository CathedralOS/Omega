#!/usr/bin/env sh
# Reject executable bootstrap gates that reach across ownership roots with
# sibling-relative paths. Local paths remain legitimate;
# cross-owner paths must come from tools/bootstrap/paths.sh.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd -P)
. "$SCRIPT_DIR/paths.sh"

# Pin canonical role ownership and the absence of the retired compiler facade,
# then prevent new topology-dependent cross-owner paths.
sh "$SCRIPT_DIR/test-paths.sh"

pattern='\.\./(alpha|beta|beta-rs|beta-lang|beta-lang-rs|beta-lang-py|gamma|delta-rs|proof-kernel|omega|lattice-corpus|psi)(/|[^A-Za-z0-9_-]|$)'

if command -v rg >/dev/null 2>&1; then
  violations=$(rg -n --glob '*.sh' --glob '*.py' \
    --glob '!**/target/**' --glob '!**/build/**' "$pattern" \
    "$OMEGA_REPO_ROOT/source" "$OMEGA_REPO_ROOT/tools/bootstrap" || true)
else
  violations=$(find "$OMEGA_REPO_ROOT/source" "$OMEGA_REPO_ROOT/tools/bootstrap" \
    \( -path '*/target' -o -path '*/build' \) -prune -o \
    -type f \( -name '*.sh' -o -name '*.py' \) \
    -exec grep -En "$pattern" {} + || true)
fi

if [ -n "$violations" ]; then
  echo "bootstrap path hygiene FAIL — cross-owner sibling paths remain:" >&2
  printf '%s\n' "$violations" >&2
  exit 1
fi

# The retired external Delta producer must not re-enter executable bridge,
# refinement, cache, or bootstrap-tool custody under either its old path or its
# former resolver variable. Git history is the archive for those wrappers.
retired_delta_pattern='OMEGA_PATH_DELTA''_RUST|source/on-ramp/rust/''delta'
if command -v rg >/dev/null 2>&1; then
  retired_delta_violations=$(rg -n "$retired_delta_pattern" \
    "$OMEGA_REPO_ROOT/source/on-ramp/omega-bootstrap/gates" \
    "$OMEGA_REPO_ROOT/source/refinement/delta-omega-bootstrap" \
    "$OMEGA_REPO_ROOT/tests/lattice/lattice-cache-deps" \
    "$OMEGA_REPO_ROOT/tools/bootstrap" || true)
else
  retired_delta_violations=$(grep -R -En "$retired_delta_pattern" \
    "$OMEGA_REPO_ROOT/source/on-ramp/omega-bootstrap/gates" \
    "$OMEGA_REPO_ROOT/source/refinement/delta-omega-bootstrap" \
    "$OMEGA_REPO_ROOT/tests/lattice/lattice-cache-deps" \
    "$OMEGA_REPO_ROOT/tools/bootstrap" || true)
fi

if [ -n "$retired_delta_violations" ]; then
  echo "bootstrap path hygiene FAIL — retired external Delta producer remains:" >&2
  printf '%s\n' "$retired_delta_violations" >&2
  exit 1
fi

echo "bootstrap path hygiene OK"
