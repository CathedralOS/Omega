#!/bin/bash
# What of a fact row already exists on a given revision?
#
# Why: two lanes did the entire `fixed_array_layout_geometry` row concurrently and
# ~90% of the work was duplicate, because neither checked whether the fixtures AND
# each golden leg already existed on the base it had just been granted. The row was
# ~90% landed already; the only real gap was nine native rows.
#
# Usage: bash tools/swarm/fact_status.sh <fact> [revision]     (revision defaults to origin/main)
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 1
fact=${1:?usage: fact_status.sh <fact> [revision]}
rev=${2:-origin/main}
git fetch origin -q 2>/dev/null

echo "== $fact @ $rev =="
# 1. Fixture directories, by exact path (never a substring: a filter for `bound` also
#    matches bounded_products, bounded_scalar_sums and boundary_provider_call).
files=$(git ls-tree -r --name-only "$rev" tests/omega 2>/dev/null | grep "/${fact}" || true)
passn=$(printf '%s\n' "$files" | grep -c '/pass/' || true)
failn=$(printf '%s\n' "$files" | grep -c '/fail/' || true)
echo "  fixtures: pass=${passn:-0} fail=${failn:-0}"
if [ "${passn:-0}" -gt 0 ]; then
  printf '%s\n' "$files" | grep '/pass/' | sed 's|.*/pass/facts/||' | sed 's|/[^/]*$||' | sort -u | head -3 | sed 's/^/    e.g. /'
fi

# 2. Each golden leg. A row can exist on one leg and not another, and `checked` is not
#    a substitute for `built ... exit:0`.
for g in tests/omega/corpus_native_macos_arm64.txt tests/omega/corpus_outcomes.txt tests/omega/corpus_both_macos_arm64.txt; do
  [ -n "$(git cat-file -e "$rev:$g" 2>/dev/null && echo ok)" ] || continue
  content=$(git show "$rev:$g" 2>/dev/null)
  built=$(printf '%s\n' "$content" | grep -c "facts/${fact}.*built .*exit:0" || true)
  checked=$(printf '%s\n' "$content" | grep -c "facts/${fact}.*checked" || true)
  rejected=$(printf '%s\n' "$content" | grep -c "facts/${fact}.*rejected" || true)
  printf '  %-42s built:%-3s checked:%-3s rejected:%-3s\n' "${g##*/}" "${built:-0}" "${checked:-0}" "${rejected:-0}"
done

echo "  ----"
echo "  a row needs 9 pass fixtures + 9 fail twins, NINE native 'built ... exit:0' rows,"
echo "  and 9 rejected rows carrying their own diagnostic. Anything already present on"
echo "  this revision is a peer's work: do NOT duplicate it, contribute only the gap."
