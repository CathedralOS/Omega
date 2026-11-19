#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Epsilon D composition"
GATE_ENVS="delta/compiler_env.sh epsilon/evaluator_env.sh omega/compiler_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

COMPOSITION_TMP=$(mktemp -d)
trap 'rm -rf -- "$COMPOSITION_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, packed closure, entry source, or packed D closure differ from the
# audited edge records.
materialize_epsilon_evaluator "$COMPOSITION_TMP/epsilon_compiler.delta"
require_epsilon_evaluator_entry_identity
materialize_delta_compiler "$COMPOSITION_TMP/delta_compiler.gamma"
materialize_delta_support "$COMPOSITION_TMP/support.bin"
materialize_omega_compiler "$COMPOSITION_TMP/omega_compiler.epsilon"
materialize_gamma_evaluator "$COMPOSITION_TMP/evaluator.exe" >/dev/null
python3 "$GATE_DIR/gate.py" "$COMPOSITION_TMP" "$@"
