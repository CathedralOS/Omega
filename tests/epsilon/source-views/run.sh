#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Epsilon source views"
GATE_ENVS="delta/compiler_env.sh epsilon/evaluator_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

SOURCE_VIEW_TMP=$(mktemp -d)
trap 'rm -rf -- "$SOURCE_VIEW_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, or packed closure differ from the audited edge records. The
# gate-local controls closure is a diagnostic artifact packed directly.
materialize_epsilon_evaluator "$SOURCE_VIEW_TMP/epsilon_compiler.delta"
materialize_delta_compiler "$SOURCE_VIEW_TMP/delta_compiler.gamma"
materialize_delta_support "$SOURCE_VIEW_TMP/support.bin"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$GATE_DIR/controls/source_views.delta.sources" "$SOURCE_VIEW_TMP/controls.delta"
materialize_gamma_evaluator "$SOURCE_VIEW_TMP/evaluator" >/dev/null
python3 "$GATE_DIR/gate.py" "$SOURCE_VIEW_TMP"
