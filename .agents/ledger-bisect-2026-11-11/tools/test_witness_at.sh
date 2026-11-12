#!/bin/bash
# Exit 0=good, 1=bad, 125=skip/build fail
set -euo pipefail
COMMIT="${1:?commit}"
WITNESS="${2:?witness}"
ROOT="${REPO_ROOT:-/agent/repos/omega}"
cd "$ROOT"
git checkout -f "$COMMIT" >/dev/null 2>&1
export BISECT_WITNESS="$WITNESS"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" \
  nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
SCRIPT="$(dirname "$0")/bisect_witness.sh"
bash "$SCRIPT"
