# Shared environment preamble for the owner-local run.sh gates. The
# caller sets GATE_DIR to its own directory and GATE_ENVS to the
# space-separated rung environment files it needs, relative to
# tools/bootstrap (e.g. GATE_ENVS="delta/compiler_env.sh"); sourcing this file
# resolves the repository root, exposes paths.sh, and sources each rung env.
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    [ "$OMEGA_REPO_ROOT" != / ] || { echo "cannot locate tools/bootstrap/paths.sh" >&2; return 1; }
    OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
for _gate_env in $GATE_ENVS; do
    . "$OMEGA_REPO_ROOT/tools/bootstrap/$_gate_env"
done
