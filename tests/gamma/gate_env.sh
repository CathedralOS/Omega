# Shared environment preamble for the tests/gamma/derivation-*/ run.sh gates:
# with GATE_DIR set to the caller's own directory and GATE_LABEL set to the
# gate's display name, resolve the repository root, source the rung
# environments, skip hosts without python3, and require a seed-capable host.
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/proofs/sources_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "$GATE_LABEL: skipped (python3 absent)"
    exit 0
}
require_seed_execution_host "$GATE_LABEL"
