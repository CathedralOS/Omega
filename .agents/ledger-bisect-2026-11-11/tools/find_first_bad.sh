#!/bin/bash
# Find earliest first-parent compiler commit after $GOOD where witness is bad (rc 1), parent good (rc 0).
set -euo pipefail
WITNESS="${1:?}"
GOOD="${2:?}"
PIN="${3:?}"
TOOL="$(cd "$(dirname "$0")" && pwd)"
ROOT="${BISECT_ROOT:-/tmp/bisect-wt}"
export BISECT_ROOT="$ROOT" BISECT_WITNESS="$WITNESS" LEDGER_PIN_FILE="${LEDGER_PIN_FILE:-/tmp/ledger-bisect-pin/corpus_outcomes.txt}"

witness_rc() { ( set +e; "$TOOL/bisect_witness.sh" >/dev/null; echo $? ); }

is_compiler_commit() {
  local sha=$1
  local parent
  parent=$(git -C "$ROOT" rev-parse --verify "${sha}^@" 2>/dev/null) || return 1
  git -C "$ROOT" diff --name-only "$parent" "$sha" | grep -qvE '^(tests/omega/corpus_outcomes\.txt|.*\.md)$'
}

while read -r sha; do
  is_compiler_commit "$sha" || continue
  parent=$(git -C "$ROOT" rev-parse --verify "${sha}^@" 2>/dev/null) || continue
  git -C "$ROOT" checkout -q "$parent"
  prc=$(witness_rc)
  [[ "$prc" -eq 0 ]] || continue
  git -C "$ROOT" checkout -q "$sha"
  brc=$(witness_rc)
  [[ "$brc" -eq 1 ]] || continue
  git -C "$ROOT" log -1 --format='%H|%s' "$sha"
  exit 0
done < <(git -C "$ROOT" rev-list --first-parent --reverse "$GOOD".."$PIN")

echo "no first bad between $GOOD and $PIN" >&2
exit 1
