#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma heap boundary: skipped (python3 absent)"
    exit 0
}
require_seed_execution_host "Gamma heap boundary"

HEAP_TMP=$(mktemp -d)
trap 'rm -rf -- "$HEAP_TMP"' EXIT HUP INT TERM
materialize_gamma_evaluator "$HEAP_TMP/evaluator.exe" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$HEAP_TMP/evaluator.exe" \
    "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" "$@"
