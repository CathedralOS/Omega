#!/usr/bin/env sh
set -eu

if [ "$#" -gt 1 ] || { [ "$#" -eq 1 ] && [ "$1" != --normalization ]; }; then
    echo "usage: $0 [--normalization]" >&2
    exit 2
fi

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta generated function census: skipped (python3 absent)"
    exit 0
}

require_seed_execution_host "Delta generated function census"

CENSUS_TMP=$(mktemp -d)
trap 'rm -rf -- "$CENSUS_TMP"' EXIT HUP INT TERM
materialize_delta_compiler "$CENSUS_TMP/compiler.gamma"
materialize_gamma_evaluator "$CENSUS_TMP/evaluator" >/dev/null
materialize_delta_support "$CENSUS_TMP/support.bin"
python3 "$GATE_DIR/gate.py" "$CENSUS_TMP" "$@"
