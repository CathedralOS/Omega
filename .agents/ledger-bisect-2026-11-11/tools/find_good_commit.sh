#!/bin/bash
# Print ledger stamp commit for a witness (short sha full sha).
set -euo pipefail
WITNESS="${1:?}"
ROOT="$(git -C "$(dirname "$0")/../../.." rev-parse --show-toplevel)"
line=$(grep -m1 "^${WITNESS} " "$ROOT/tests/omega/corpus_outcomes.txt" || true)
if [[ -z "$line" ]]; then
  echo "UNKNOWN"
  exit 1
fi
stamp=$(echo "$line" | grep -oE '@[0-9a-f]{12}' | head -1 | tr -d '@')
git -C "$ROOT" rev-parse "$stamp" 2>/dev/null || echo "$stamp"
