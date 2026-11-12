#!/bin/bash
# Find a first-parent ancestor where witness matches ledger (exit 0).
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
TOOL="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$TOOL" rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"

stamps=()
while read -r line; do
  [[ "$line" =~ @( [0-9a-f]+|[0-9a-f]+)(\+wt)? ]] && stamps+=("${BASH_REMATCH[1]}") || true
done < <(git log --first-parent -S "$WITNESS checked" --format=%H -- tests/omega/corpus_outcomes.txt | head -20)
while read -r line; do
  stamp=$(git show "$ROOT/tests/omega/corpus_outcomes.txt" 2>/dev/null | rg "^$WITNESS " | rg -o '@[0-9a-f]+' | head -1 | tr -d '@' || true)
done

ledger_stamp=$(git show "HEAD:tests/omega/corpus_outcomes.txt" | rg "^$WITNESS " | rg -o '@[0-9a-f]+(\+wt)?' | head -1 | sed 's/^@//;s/+wt$//' || true)
candidates=()
[[ -n "$ledger_stamp" ]] && candidates+=("$ledger_stamp")
mapfile -t rec < <(git log --first-parent --format=%H -S "$WITNESS checked" -- tests/omega/corpus_outcomes.txt | head -10)
candidates+=("${rec[@]}")
mapfile -t rec2 < <(git log --first-parent --format=%H -S "$WITNESS built" -- tests/omega/corpus_outcomes.txt | head -5)
candidates+=("${rec2[@]}")

seen=""
for sha in "${candidates[@]}"; do
  [[ -z "$sha" ]] && continue
  full=$(git rev-parse --verify "${sha}^{commit}" 2>/dev/null) || continue
  [[ " $seen " == *" $full "* ]] && continue
  seen="$seen $full"
  git checkout -q "$full"
  set +e
  "$TOOL/bisect_witness.sh" >/dev/null
  rc=$?
  set -e
  subj=$(git log -1 --format=%s "$full")
  echo "probe $full rc=$rc $subj" >&2
  if [[ "$rc" -eq 0 ]]; then
    git checkout -q "$PIN"
    echo "$full"
    exit 0
  fi
done

# Walk first-parent up to 4000 commits from PIN^
cur="$(git rev-parse "$PIN")"
for _ in $(seq 1 4000); do
  cur="$(git rev-parse "$cur^" 2>/dev/null)" || break
  git checkout -q "$cur"
  set +e
  "$TOOL/bisect_witness.sh" >/dev/null
  rc=$?
  set -e
  if [[ "$rc" -eq 0 ]]; then
    git checkout -q "$PIN"
    echo "$cur"
    exit 0
  fi
done
git checkout -q "$PIN"
exit 1
