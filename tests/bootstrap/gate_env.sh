# Shared environment preamble for the tests/bootstrap/*.sh gates: with
# TEST_DIR set to the caller's own directory and GATE_ENVS to the rung
# environment files relative to tools/bootstrap, resolve the repository
# root, expose paths.sh, and source each rung env.
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
for _gate_env in $GATE_ENVS; do
    . "$OMEGA_REPO_ROOT/tools/bootstrap/$_gate_env"
done
