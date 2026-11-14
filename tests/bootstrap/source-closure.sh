#!/usr/bin/env sh
# Ordered source-closure packing and refusal. The checks live in the xtask
# source-closure tests; this gate runs them.
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
cd "$OMEGA_REPO_ROOT"

command -v mbx >/dev/null 2>&1 || {
  echo "source closure: skipped (mbx absent)"
  exit 0
}

mbx test -p xtask source_closure < /dev/null
