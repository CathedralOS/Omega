#!/bin/bash
# Walk first-parent from PIN until witness test returns 0 (skips ledger-only via bisect script).
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:?}"
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
TOOL="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
cd "$ROOT"
cur="$PIN"
steps=0
max=8000
while ((steps < max)); do
  git checkout -q "$cur"
  if parents=$(git rev-list --first-parent -n 1 "$cur"^ 2>/dev/null); then :; fi
  parent=$(git rev-parse "$cur^1" 2>/dev/null || true)
  rc=0
  BISECT_WITNESS="$WITNESS" "$TOOL" || rc=$?
  if [[ "$rc" -eq 0 ]]; then
    echo "$cur"
    exit 0
  fi
  if [[ "$rc" -ne 1 && "$rc" -ne 125 ]]; then
    echo "unexpected rc $rc at $cur" >&2
    exit 1
  fi
  if [[ -z "$parent" ]]; then
    echo "no good commit found" >&2
    exit 1
  fi
  cur="$parent"
  steps=$((steps + 1))
  if ((steps % 50 == 0)); then echo "walked $steps, at $cur" >&2; fi
done
echo "exceeded max steps" >&2
exit 1
