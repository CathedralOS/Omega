#!/usr/bin/env sh
# Reject executable bootstrap gates that reach across the historical flat
# compiler tree with sibling-relative paths.  Local paths remain legitimate;
# cross-owner paths must come from bootstrap/paths.sh.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd -P)
. "$SCRIPT_DIR/paths.sh"

# Keep the role manifest and temporary compatibility entry points pinned while
# this static scan prevents new topology-dependent cross-owner paths.
sh "$SCRIPT_DIR/test-paths.sh"

pattern='\.\./(alpha|beta|beta-rs|beta-lang|beta-lang-rs|beta-lang-py|gamma|delta-rs|proof-kernel|omega|omega-rs|lattice-corpus|psi-rs)(/|[^A-Za-z0-9_-]|$)'

if command -v rg >/dev/null 2>&1; then
  violations=$(rg -n --glob '*.sh' --glob '*.py' \
    --glob '!**/target/**' --glob '!**/build/**' "$pattern" \
    "$OMEGA_PATH_COMPILER_ROOT" "$OMEGA_REPO_ROOT/bootstrap" || true)
else
  violations=$(find "$OMEGA_PATH_COMPILER_ROOT" "$OMEGA_REPO_ROOT/bootstrap" \
    \( -path '*/target' -o -path '*/build' \) -prune -o \
    -type f \( -name '*.sh' -o -name '*.py' \) \
    -exec grep -En "$pattern" {} + || true)
fi

if [ -n "$violations" ]; then
  echo "bootstrap path hygiene FAIL — cross-owner sibling paths remain:" >&2
  printf '%s\n' "$violations" >&2
  exit 1
fi

echo "bootstrap path hygiene OK"
