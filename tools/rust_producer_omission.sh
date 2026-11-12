#!/usr/bin/env sh
# Developer-only dependency audit. Rust builds this verifier, not a bootstrap
# stage; the source-construction invocation never calls it.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd -P)
AUDITOR=${OMEGA_XTASK_BIN:-"$OMEGA_REPO_ROOT/target/debug/xtask"}
[ -x "$AUDITOR" ] || {
    echo "Rust producer omission audit: build the developer verifier with mbx build -p xtask" >&2
    exit 2
}

# Explicit declarations serve source-archive fixtures too: the verifier's
# build-time checkout must not choose the input root under review.
if [ "$#" -eq 0 ]; then
    cd "$OMEGA_REPO_ROOT"
    manifests=$(find bootstrap/0_alpha bootstrap/1_beta bootstrap/2_gamma \
        bootstrap/3_delta bootstrap/4_epsilon bootstrap/5_omega bootstrap/proofs \
        -type f -name '*.sources' | LC_ALL=C sort)
    steps=$(find tools/bootstrap -type f -name '*.sh' \
        ! -name 'check-chain-hygiene.sh' | LC_ALL=C sort)
    [ -n "$manifests" ] && [ -n "$steps" ] || {
        echo "Rust producer omission audit: empty manifest or step inventory" >&2
        exit 2
    }
    for manifest in $manifests; do
        set -- "$@" --manifest "$manifest"
    done
    for step in $steps; do
        set -- "$@" --steps "$step"
    done
fi
exec "$AUDITOR" rust-producer-omission --root "$OMEGA_REPO_ROOT" "$@" --require omitted
