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

# The nine transport places. A fixture family is named
# `<fact>_through_<place><suffix>`; resolving the suffix against this closed set
# is what keeps the `<fact>` component exact.
places='local|field|parameter|call_result|match_join|const_name|other_state|other_machine|target_body'

echo "== $fact @ $rev =="
# 1. Fixture directories, by exact path component. Both halves of the name are
#    matched exactly: the fact component is whatever precedes the first
#    `_through_`, and the suffix must begin with one of the nine place names.
#    A substring filter is wrong in both directions: `length` also matches the
#    unrelated `bound`/_length_/ families, and `bound` also matches
#    `bounded_products`, `bounded_scalar_sums` and `boundary_provider_call`.
#    Counting files instead of directories was wrong too: it reported a nine-place
#    row as 27 pass fixtures, because each family holds three files.
# Path components: tests/omega/<leg>/facts/<family>/<file>
entries=$(git ls-tree -r --name-only "$rev" tests/omega/pass/facts tests/omega/fail/facts 2>/dev/null \
  | awk -F/ 'NF >= 6 { print $3 "/" $5 }' | sort -u)
# Real naming shapes on main, all matched with both components exact:
#   pass/<fact>_through_<place>_exit
#   fail/<fact>_through_<place>_rejected
#   fail/<fact>_<variant>_<place>        (variant: unproved, held, ...)
matched=$(printf '%s\n' "$entries" \
  | grep -E "^(pass/${fact}_through|fail/${fact}_through)_?(${places})(_|/|$)|^fail/${fact}_[a-z_]*_(${places})(_|/|$)" || true)
passn=$(printf '%s\n' "$matched" | grep -c '^pass/' || true)
failn=$(printf '%s\n' "$matched" | grep -c '^fail/' || true)
echo "  fixtures: pass=${passn:-0} fail=${failn:-0} (directories, matched on the exact path component)"
if [ "${passn:-0}" -gt 0 ]; then
  printf '%s\n' "$matched" | grep '^pass/' | sed 's|^pass/||' | sort | head -3 | sed 's/^/    e.g. /'
fi

# 2. Each golden leg, counted over this fact's own fixture rows only. The row's
#    first field is a repo-relative fixture path, so the match anchors on the
#    whole component rather than on any line that happens to contain the name.
#    `checked` is not a substitute for `built ... exit:0`.
for g in tests/omega/corpus_native_macos_arm64.txt tests/omega/corpus_outcomes.txt tests/omega/corpus_both_macos_arm64.txt; do
  [ -n "$(git cat-file -e "$rev:$g" 2>/dev/null && echo ok)" ] || continue
  content=$(git show "$rev:$g" 2>/dev/null)
  rows=$(printf '%s\n' "$content" \
    | awk -v f="$fact" -v p="$places" '
        $1 ~ ("^pass/facts/" f "_through_(" p ")(_|$)") { print; next }
        $1 ~ ("^fail/facts/" f "_through_(" p ")(_|$)") { print; next }
        $1 ~ ("^fail/facts/" f "_[a-z_]*(" p ")(_|$)") { print }')
  built=$(printf '%s\n' "$rows" | grep -c ' built .*exit:0' || true)
  checked=$(printf '%s\n' "$rows" | grep -c ' checked' || true)
  rejected=$(printf '%s\n' "$rows" | grep -c ' rejected' || true)
  printf '  %-42s built:%-3s checked:%-3s rejected:%-3s\n' "${g##*/}" "${built:-0}" "${checked:-0}" "${rejected:-0}"
done

echo "  ----"
echo "  a row needs 9 pass fixtures + 9 fail twins, NINE native 'built ... exit:0' rows,"
echo "  and 9 rejected rows carrying their own diagnostic. Anything already present on"
echo "  this revision is a peer's work: do NOT duplicate it, contribute only the gap."