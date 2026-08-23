#!/usr/bin/env sh
# Compatibility entry point; canonical owner: bootstrap/assurance/refinement/beta.
set -eu
OMEGA_COMPAT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$OMEGA_COMPAT_DIR/../.." && pwd -P)
exec sh "$OMEGA_REPO_ROOT/bootstrap/assurance/refinement/beta/symbolic-loops.sh" "$@"
