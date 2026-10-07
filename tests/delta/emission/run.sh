#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Delta emission"
GATE_ENVS="delta/compiler_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

EMISSION_TMP=$(mktemp -d)
trap 'rm -rf -- "$EMISSION_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$GATE_DIR/controls/emission.gamma.sources" "$EMISSION_TMP/controls.gamma"
# The bound member closure is checked against its audited record; the gate's
# packed controls entry packs on top of those bound members.
require_delta_compiler_identity
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$EMISSION_TMP/compiler.gamma" \
    --prefix "$EMISSION_TMP/controls.gamma"
materialize_gamma_evaluator "$EMISSION_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$EMISSION_TMP"
