#!/bin/bash
# After GOOD is known: first compiler commit on first-parent where witness is bad (exit 1).
set -euo pipefail
GOOD="${1:?good}"
PIN="${2:?pin}"
WITNESS="${3:?witness}"
SLUG="$(echo "$WITNESS" | tr '/' '_')"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
export BISECT_WITNESS_PIN="$ROOT/.agents/ledger-bisect-2026-11-11/pin-witness/${SLUG}.json"
[[ -f "$BISECT_WITNESS_PIN" ]] || { echo "missing pin json" >&2; exit 1; }
for c in $(git rev-list --first-parent --reverse "$GOOD".."$PIN"); do
  mapfile -t CH < <(git diff --name-only "$c^" "$c" 2>/dev/null)
  skip=1
  for f in "${CH[@]}"; do case "$f" in tests/omega/corpus_outcomes.txt|*.md) ;; *) skip=0 ;; esac; done
  [[ $skip -eq 1 && ${#CH[@]} -gt 0 ]] && continue
  git -c advice.detachedHead=false checkout -f "$c" >/dev/null 2>&1
  code=0
  bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" >/dev/null 2>&1 || code=$?
  if [[ $code -eq 1 ]]; then
    git -c advice.detachedHead=false checkout -f work/ledger-bisect-part-27 >/dev/null 2>&1 || git checkout -f "$PIN"
    echo "$c"
    exit 0
  fi
done
git -c advice.detachedHead=false checkout -f work/ledger-bisect-part-27 >/dev/null 2>&1 || git checkout -f "$PIN"
exit 1
