#!/usr/bin/env sh
set -eu
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
stamp_seed "$1/program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$1/program.exe"
exec "$1/program.exe"