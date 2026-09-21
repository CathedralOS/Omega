#!/usr/bin/env sh
# Rust producer retirement gate: the canonical bootstrap input set must
# omit the Rust producer. Audits every produced closure manifest under
# bootstrap/ plus the shared orchestration step surface under
# tools/bootstrap/ with `tools/rust_producer_omission.py --require omitted`.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd -P)
export OMEGA_REPO_ROOT

command -v python3 >/dev/null 2>&1 || {
    echo "Rust producer omission: skipped (python3 absent)"
    exit 0
}
[ "$#" -eq 0 ] || { echo "usage: rust_producer_omission.sh" >&2; exit 2; }

cd "$OMEGA_REPO_ROOT"
manifests=$(find bootstrap -type f -name '*.sources' | LC_ALL=C sort)
[ -n "$manifests" ] || {
    echo "Rust producer omission: no bootstrap closure manifests found" >&2
    exit 1
}
steps=$(find tools/bootstrap -type f -name '*.sh' | LC_ALL=C sort)
[ -n "$steps" ] || {
    echo "Rust producer omission: no bootstrap step surface found" >&2
    exit 1
}

set --
for manifest in $manifests; do
    set -- "$@" --manifest "$manifest"
done
for step in $steps; do
    set -- "$@" --steps "$step"
done

python3 "$OMEGA_REPO_ROOT/tools/rust_producer_omission.py" "$@" \
    --root "$OMEGA_REPO_ROOT" --require omitted
