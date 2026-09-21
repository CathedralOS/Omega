#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon array storage: skipped (python3 absent)"
    exit 0
}

require_seed_execution_host "Epsilon array storage"

ARRAY_TMP=$(mktemp -d)
trap 'rm -rf -- "$ARRAY_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, or packed closure differ from the audited edge records.
materialize_epsilon_evaluator "$ARRAY_TMP/epsilon_compiler.delta"
materialize_delta_compiler "$ARRAY_TMP/delta_compiler.gamma"
materialize_delta_support "$ARRAY_TMP/support.bin"
materialize_gamma_evaluator "$ARRAY_TMP/evaluator" >/dev/null
python3 "$GATE_DIR/gate.py" "$ARRAY_TMP" \
    "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER" \
    "$GATE_DIR"
