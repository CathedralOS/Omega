#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/omega/compiler_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Omega executable: skipped (python3 absent)"
    exit 0
}

require_seed_execution_host "Omega executable"

OUTPUT_DIR=${OMEGA_EXECUTABLE_BUILD_DIR:-"$OMEGA_REPO_ROOT/build/omega-executable"}
mkdir -p "$OUTPUT_DIR"
# Bound materializers refuse before writing when the canonical entry,
# manifest, members, packed closure, or composed record differ from the
# audited edge records.
materialize_delta_compiler "$OUTPUT_DIR/delta_compiler.gamma"
materialize_delta_support "$OUTPUT_DIR/support.bin"
materialize_epsilon_evaluator "$OUTPUT_DIR/epsilon_compiler.delta"
materialize_omega_compiler "$OUTPUT_DIR/omega_compiler.epsilon"
require_omega_executable_entries_identity
materialize_gamma_evaluator "$OUTPUT_DIR/evaluator.exe" >/dev/null
python3 "$GATE_DIR/gate.py" "$OUTPUT_DIR" "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER" "$@"