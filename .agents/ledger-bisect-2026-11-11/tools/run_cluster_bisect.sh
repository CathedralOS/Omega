#!/bin/bash
# Bisect one witness; prints: breaking_commit pr mechanism (tab-separated) on success.
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
cd "$ROOT"
TOOLS="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
export BISECT_WITNESS="$WITNESS"

# Stability at PIN
for _ in 1 2 3; do
  git checkout -q "$PIN"
  BISECT_WITNESS="$WITNESS" "$TOOLS/bisect_witness.sh" || bad=$?
  bad=${bad:-0}
  [[ "$bad" -eq 1 ]] || { echo "PIN unstable: rc=$bad" >&2; exit 2; }
done

GOOD="$("$TOOLS/find_good_first_parent.sh" "$WITNESS" "$PIN")"
for _ in 1 2 3; do
  git checkout -q "$GOOD"
  BISECT_WITNESS="$WITNESS" "$TOOLS/bisect_witness.sh" || goodrc=$?
  goodrc=${goodrc:-0}
  [[ "$goodrc" -eq 0 ]] || { echo "GOOD unstable at $GOOD rc=$goodrc" >&2; exit 2; }
done

git checkout -q "$PIN"
git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$GOOD"
log=$(git bisect run "$TOOLS/bisect_witness.sh" 2>&1) || true
echo "$log" | tail -20 >&2
first_bad=$(echo "$log" | sed -n 's/.*first bad commit: \(.*\)/\1/p' | head -1)
if [[ -z "$first_bad" ]]; then
  first_bad=$(echo "$log" | sed -n 's/^\[.*\] \(.*\)$/\1/p' | tail -1)
fi
sha=$(git rev-parse "$first_bad^{commit}" 2>/dev/null || git rev-parse "$first_bad")
subj=$(git log -1 --format=%s "$sha")
pr=$(echo "$subj" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
mech=$(git show --stat --oneline "$sha" | head -3 | tr '\n' ' ')
git bisect reset >/dev/null 2>&1 || true
git checkout -q "$PIN"
printf '%s\t%s\t%s\t%s\n' "$sha" "$pr" "$mech" "deterministic"
