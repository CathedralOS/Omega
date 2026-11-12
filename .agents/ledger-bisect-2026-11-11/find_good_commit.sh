#!/bin/bash
# Newest first-parent ancestor of PIN where witness returns good (fixed PIN expectations).
set -euo pipefail
PIN="${1:?PIN}"
SPEC="${2:?spec json}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS_SPEC="$SPEC"
DIR="$(cd "$(dirname "$0")" && pwd)"
BRANCH="$(git branch --show-current 2>/dev/null || true)"

test_commit() {
  git checkout -q "$1"
  local rc=0
  "$DIR/bisect_witness.sh" || rc=$?
  [[ "$rc" -eq 0 ]]
}

restore() {
  [[ -n "$BRANCH" ]] && git checkout -q "$BRANCH" || git checkout -q "$PIN"
}

# Recent fix/re-break (latest good within small first-parent window).
for c in $(git rev-list --first-parent "${PIN}^1" | head -5); do
  if test_commit "$c"; then
    echo "$c"
    restore
    exit 0
  fi
done

# Older anchor commits (compiler-good baselines).
for c in de25db89edf196dc1bf697a9c34886669d66b694 759c80404428f6f2bbf8f0f5ec1f6e0c2b8e4e56; do
  if git cat-file -e "$c^{commit}" 2>/dev/null && test_commit "$c"; then
    echo "$c"
    restore
    exit 0
  fi
done

restore
exit 1
