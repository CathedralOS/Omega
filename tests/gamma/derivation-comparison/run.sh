#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/proofs/sources_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Derivation comparison: skipped (python3 absent)"
    exit 0
}
require_seed_execution_host "Derivation comparison"

COMPARISON_TMP=$(mktemp -d)
trap 'rm -rf -- "$COMPARISON_TMP"' EXIT HUP INT TERM
# The bound materializer refuses before writing when the canonical manifest,
# members, or packed member closure differ from the audited proof record.
# The gate's bound diagnostic prefix and per-vector entries pack on top of
# those bound members.
materialize_derivation_checker "$COMPARISON_TMP/implementation.gamma"
require_derivation_comparison_prefixes_identity
materialize_gamma_evaluator "$COMPARISON_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$COMPARISON_TMP"
