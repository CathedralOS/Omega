#!/bin/bash
# Verify bisect witness: exit 0 pass at GOOD, 1 bad at BAD
set -euo pipefail
WITNESS="${1:?witness}"
GOOD="${2:?good sha}"
BAD="${3:?bad sha}"
ROOT="/agent/repos/omega"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
cd "$ROOT"
for commit in "$GOOD" "$BAD"; do
  git checkout -q "$commit"
  # force fresh build
  rm -rf "$ROOT/target"
  export BISECT_WITNESS="$WITNESS"
  set +e
  "$SCRIPT"
  rc=$?
  set -e
  echo "commit=$commit rc=$rc"
done
git checkout -q work/ledger-bisect-part-5
