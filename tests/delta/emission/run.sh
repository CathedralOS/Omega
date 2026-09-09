#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta emission: skipped (python3 absent)"
    exit 0
}

EMISSION_TMP=$(mktemp -d)
trap 'rm -rf -- "$EMISSION_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$GATE_DIR/controls/emission.gamma.sources" "$EMISSION_TMP/controls.gamma"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$EMISSION_TMP/compiler.gamma" \
    --prefix "$EMISSION_TMP/controls.gamma"
materialize_gamma_evaluator "$EMISSION_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$EMISSION_TMP"
