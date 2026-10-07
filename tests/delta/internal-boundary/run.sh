#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Delta internal boundary"
GATE_ENVS="delta/compiler_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

INTERNAL_TMP=$(mktemp -d)
trap 'rm -rf -- "$INTERNAL_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$GATE_DIR/controls.gamma.sources" "$INTERNAL_TMP/controls.gamma"
# The bound member closure is checked against its audited record; the gate's
# packed controls entry packs on top of those bound members.
require_delta_compiler_identity
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$INTERNAL_TMP/diagnostic.gamma" \
    --prefix "$INTERNAL_TMP/controls.gamma"
materialize_delta_compiler "$INTERNAL_TMP/canonical.gamma"
materialize_delta_support "$INTERNAL_TMP/support.bin"
materialize_gamma_evaluator "$INTERNAL_TMP/evaluator.exe" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$INTERNAL_TMP"
