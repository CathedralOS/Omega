#!/bin/bash
# Walk first-parent from GOOD toward PIN; first commit with PIN-repro rc=1 (skip ledger-only).
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:?}"
GOOD="${3:?}"
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
TOOL="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
cd "$ROOT"
first_bad=""
mapfile -t chain < <(git rev-list --first-parent "$PIN" --not "$GOOD" | tac)
for c in "${chain[@]}"; do
  git checkout -q "$c"
  rc=0
  "$TOOL" || rc=$?
  if [[ "$rc" -eq 1 ]]; then
    first_bad="$c"
    break
  fi
done
if [[ -z "$first_bad" ]]; then
  echo "no PIN-reproducing bad commit in range" >&2
  exit 1
fi
git log -1 --oneline "$first_bad"
echo "$first_bad"
