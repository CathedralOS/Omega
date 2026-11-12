#!/bin/bash
# Walk first-parent ancestors from PIN until witness matches good_status (exit 0).
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
cur="$PIN"
while [[ -n "$cur" ]]; do
  git checkout -q "$cur"
  set +e
  bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" >/dev/null 2>&1
  rc=$?
  set -e
  if [[ "$rc" -eq 0 ]]; then
    echo "$cur"
    git log -1 --oneline "$cur"
    exit 0
  fi
  if [[ "$rc" -eq 1 ]]; then
    cur="$(git rev-parse "${cur}^1" 2>/dev/null || true)"
    continue
  fi
  cur="$(git rev-parse "${cur}^1" 2>/dev/null || true)"
done
echo "find_good_commit: no good commit for $WITNESS" >&2
exit 1
