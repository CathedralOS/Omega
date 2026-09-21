#!/usr/bin/env sh
set -eu
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# The stamped program is the audited Alpha container. Hosts that cannot exec
# the selected container refuse (exit 2) rather than crash on the exec below;
# the audited host matrix lives in seed_env.sh.
require_seed_execution_host "Omega executable"

stamp_seed "$1/program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$1/program.exe"
exec "$1/program.exe"