#!/bin/bash
# One-line mechanism from first-parent breaking commit (compiler paths only).
set -euo pipefail
commit="${1:?}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
files=$(git diff-tree --no-commit-id --name-only -r "$commit" | grep -vE '^tests/omega/corpus_outcomes\.txt$|\.md$' | head -3)
subject=$(git log -1 --format=%s "$commit")
if [[ -z "$files" ]]; then
  echo "ledger-only or docs-only commit; no compiler mechanism in diff"
  exit 0
fi
echo "$subject — touched $(echo "$files" | paste -sd, -)"
