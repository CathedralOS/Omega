#!/bin/bash
# Find newest first-parent ancestor of BAD where witness passes (bisect_test exit 0).
set -euo pipefail
WITNESS="${1:?}"
BAD="${2:-741270df60f7}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
export BISECT_WITNESS="$WITNESS"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"

test_commit() {
  local c="$1"
  git checkout -q "$c"
  "$SCRIPT"
}

# bad end must fail
rc=$(test_commit "$BAD" || true)
if [[ "$rc" -eq 0 ]]; then
  echo "ERROR: BAD $BAD passes witness" >&2
  exit 2
fi

# exponential walk back on first-parent
CUR="$BAD"
GOOD=""
for i in $(seq 1 40); do
  PARENT=$(git rev-parse "${CUR}^" 2>/dev/null || true)
  [[ -z "$PARENT" ]] && break
  rc=$(test_commit "$PARENT" || true)
  echo "try $i $(git log -1 --oneline "$PARENT") -> exit $rc" >&2
  if [[ "$rc" -eq 0 ]]; then
    GOOD="$PARENT"
    break
  fi
  CUR="$PARENT"
done

if [[ -z "$GOOD" ]]; then
  echo "ERROR: no good found within 40 parents of $BAD" >&2
  git checkout -q work/ledger-bisect-part-30
  exit 2
fi

echo "$GOOD"
git checkout -q work/ledger-bisect-part-30
