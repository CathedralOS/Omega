#!/bin/bash
set -euo pipefail
PARENT="${1:?parent}"
FIRST_BAD="${2:?first_bad}"
shift 2
export LEDGER_BISECT_REPO=/agent/repos/omega
export LEDGER_BISECT_TOOLS=/tmp/part32-bisect-tools/tools
for fix in "$@"; do
  echo "=== $fix ==="
  echo -n "parent: "
  /tmp/part32-bisect-tools/tools/verify_at_commit.sh "$PARENT" "$fix" || true
  echo -n "first_bad: "
  /tmp/part32-bisect-tools/tools/verify_at_commit.sh "$FIRST_BAD" "$fix" || true
done
cd "$LEDGER_BISECT_REPO"
git checkout -q work/ledger-bisect-part-32
