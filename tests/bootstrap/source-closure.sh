#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"

command -v python3 >/dev/null 2>&1 || {
  echo "source closure: skipped (python3 absent)"
  exit 0
}

python3 "$TEST_DIR/source-closure.py"
