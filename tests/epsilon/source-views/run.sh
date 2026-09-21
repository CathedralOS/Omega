#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon source views: skipped (python3 absent)"
    exit 0
}

require_seed_execution_host "Epsilon source views"

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
