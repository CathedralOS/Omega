#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Gamma heap boundary"
GATE_ENVS="alpha/seed_env.sh gamma/evaluator_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

HEAP_TMP=$(mktemp -d)
trap 'rm -rf -- "$HEAP_TMP"' EXIT HUP INT TERM
materialize_gamma_evaluator "$HEAP_TMP/evaluator.exe" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$HEAP_TMP/evaluator.exe" \
    "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" "$@"
