# Shared environment preamble for the tests/bootstrap/*/ run.sh gates: with
# GATE_DIR set to the caller's own directory, resolve the repository root and
# source the rung environments every gate needs. Seed support arrives twice —
# explicitly here and transitively through epsilon/omega — and is idempotent.
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/omega/compiler_env.sh"
