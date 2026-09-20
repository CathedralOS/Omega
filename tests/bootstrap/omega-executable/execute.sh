#!/usr/bin/env sh
set -eu
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

# The stamped program is the audited Alpha container: a Mach-O seed on macOS
# arm64, the Windows PE seed elsewhere. Hosts that cannot exec the selected
# container refuse (exit 2) rather than crash on the exec below.
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Omega executable: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

stamp_seed "$1/program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$1/program.exe"
exec "$1/program.exe"