#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Epsilon array storage"
GATE_ENVS="delta/compiler_env.sh epsilon/evaluator_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

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
