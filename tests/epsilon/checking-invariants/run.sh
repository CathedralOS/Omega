#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon checking invariants: skipped (python3 absent)"
    exit 0
}

CHECKING_TMP=$(mktemp -d)
trap 'rm -rf -- "$CHECKING_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$CHECKING_TMP/epsilon_compiler.delta"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$CHECKING_TMP/delta_compiler.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$GATE_DIR/checking_invariants.delta.sources" "$CHECKING_TMP/controls.delta"
materialize_gamma_evaluator "$CHECKING_TMP/evaluator" >/dev/null
python3 "$GATE_DIR/gate.py" "$CHECKING_TMP"
