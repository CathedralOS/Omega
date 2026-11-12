#!/bin/bash
# Find newest first-parent ancestor of BAD where witness passes (bisect_test exit 0).
set -euo pipefail
WITNESS="${1:?}"
BAD="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
export OMEGA_REPO="/agent/repos/omega"
ROOT="$OMEGA_REPO"
cd "$ROOT"
export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
export BISECT_WITNESS="$WITNESS"
export OMEGA_BISECT_TOOLS="${OMEGA_BISECT_TOOLS:-/tmp/ledger-bisect-part-30-tools}"
SCRIPT="$OMEGA_BISECT_TOOLS/bisect_witness.sh"

test_commit() {
  local c="$1"
  git checkout -qf "$c"
  set +e
  "$SCRIPT"
  local r=$?
  set -e
  return "$r"
}

# bad end must fail
set +e
test_commit "$BAD"
rc=$?
set -e
if [[ "$rc" -eq 0 ]]; then
  echo "ERROR: BAD $BAD passes witness (rc=0)" >&2
  exit 2
fi

CUR="$BAD"
GOOD=""
for i in $(seq 1 60); do
  PARENT=$(git rev-parse "${CUR}^" 2>/dev/null || true)
  [[ -z "$PARENT" ]] && break
  set +e
  test_commit "$PARENT"
  rc=$?
  set -e
  echo "try $i $(git log -1 --oneline "$PARENT") -> exit $rc" >&2
  if [[ "$rc" -eq 0 ]]; then
    GOOD="$PARENT"
    break
  fi
  CUR="$PARENT"
done

if [[ -z "$GOOD" ]]; then
  echo "ERROR: no good found within 60 parents of $BAD" >&2
  git checkout -qf work/ledger-bisect-part-30
  exit 2
fi

echo "$GOOD"
git checkout -qf work/ledger-bisect-part-30
