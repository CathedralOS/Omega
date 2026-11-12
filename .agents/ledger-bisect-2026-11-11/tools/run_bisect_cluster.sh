#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
TOOL="$(cd "$(dirname "$0")" && pwd)"
ROOT="${BISECT_ROOT:-$(git -C "$TOOL" rev-parse --show-toplevel)}"
cd "$ROOT"
export BISECT_ROOT="$ROOT" BISECT_WITNESS="$WITNESS"
export LEDGER_PIN_FILE=/tmp/ledger-bisect-pin/corpus_outcomes.txt

witness_rc() {
  ( set +e; "$TOOL/bisect_witness.sh" >/dev/null; echo $? )
}

is_ledger_only_commit() {
  local sha=$1
  parent=$(git rev-parse --verify "${sha}^@" 2>/dev/null) || return 1
  mapfile -t touched < <(git diff --name-only "$parent" "$sha")
  ((${#touched[@]} == 0)) && return 1
  local f
  for f in "${touched[@]}"; do
    [[ "$f" == "tests/omega/corpus_outcomes.txt" ]] && continue
    [[ "$f" == *.md ]] && continue
    return 1
  done
  return 0
}

mechanism_for() {
  local sha=$1
  [[ -n "$sha" ]] || { echo "unknown"; return; }
  if is_ledger_only_commit "$sha"; then
    echo "ledger-only commit; skipped"
    return
  fi
  subject=$(git log -1 --format=%s "$sha")
  mapfile -t code < <(git diff --name-only "${sha}^@" "$sha" | grep -E '\.(rs|omg)$' | grep -v corpus_outcomes | head -3)
  if ((${#code[@]} > 0)); then
  printf '%s ({ %s })' "$subject" "$(IFS=,; echo "${code[*]}")"
  else
  printf '%s' "$subject"
  fi
}

git checkout -q "$PIN"
pin_rc=$(witness_rc)
[[ "$pin_rc" -eq 1 ]] || {
  printf '{"witness":"%s","error":"PIN not bad under frozen baseline","rc":%s}\n' "$WITNESS" "$pin_rc"
  exit 2
}

good=""
fixture_path="tests/omega/${WITNESS}"
intro=$(git log --diff-filter=A --format=%H -1 -- "$fixture_path" 2>/dev/null || true)
if [[ -n "$intro" ]]; then
  git checkout -q "$intro"
  rc=$(witness_rc)
  [[ "$rc" -eq 0 ]] && good="$intro"
fi

cur="$PIN"
for _ in $(seq 1 20000); do
  cur=$(git rev-parse --verify "${cur}^@" 2>/dev/null) || break
  git checkout -q "$cur"
  rc=$(witness_rc)
  if [[ "$rc" -eq 0 ]]; then
    good="$cur"
    break
  fi
  # 125 = build/transient skip; retry once before walking further.
  if [[ "$rc" -eq 125 ]]; then
    sleep 5
    rc=$(witness_rc)
    [[ "$rc" -eq 0 ]] && good="$cur" && break
    [[ "$rc" -eq 125 ]] && continue
  fi
done
git checkout -q "$PIN"
[[ -n "$good" ]] || {
  printf '{"witness":"%s","error":"no good first-parent ancestor"}\n' "$WITNESS"
  exit 3
}

flaky_good=0
flaky_bad=0
for _ in 1 2 3; do
  git checkout -q "$good"
  r=$(witness_rc)
  [[ "$r" -ne 0 ]] && flaky_good=1
done
for _ in 1 2 3; do
  git checkout -q "$PIN"
  r=$(witness_rc)
  [[ "$r" -ne 1 ]] && flaky_bad=1
done
git checkout -q "$PIN"

export BISECT_ROOT="$ROOT" BISECT_WITNESS="$WITNESS"
mapfile -t commits < <(
  while read -r sha; do
    is_ledger_only_commit "$sha" && continue
    echo "$sha"
  done < <(git rev-list --first-parent --reverse "$good".."$PIN")
)
first_bad=""
bisect_rc=1
if ((${#commits[@]} == 0)); then
  :
elif ((${#commits[@]} == 1)); then
  first_bad="${commits[0]}"
  bisect_rc=0
else
  lo=0
  hi=$((${#commits[@]} - 1))
  : >/tmp/bisect_run.log
  while ((hi - lo > 1)); do
    mid=$(((lo + hi) / 2))
    git checkout -q "${commits[mid]}"
    rc=$(witness_rc)
    echo "probe ${commits[mid]} rc=$rc" >>/tmp/bisect_run.log
    if [[ "$rc" -eq 0 ]]; then
      lo=$mid
    elif [[ "$rc" -eq 1 ]]; then
      hi=$mid
    else
      hi=$mid
    fi
  done
  first_bad="${commits[hi]}"
  bisect_rc=0
fi
git checkout -q "$PIN"

if [[ -z "$first_bad" ]]; then
  printf '{"witness":"%s","good":"%s","error":"bisect produced no first_bad","flaky_good":%s,"flaky_bad":%s,"bisect_rc":%s}\n' \
    "$WITNESS" "$good" "$flaky_good" "$flaky_bad" "$bisect_rc"
  exit 5
fi

mech=$(mechanism_for "$first_bad")
pr=$(echo "$mech" | sed -n 's/.*(#\([0-9][0-9]*\)).*/\1/p')
printf '{"witness":"%s","good":"%s","first_bad":"%s","pr":"%s","mechanism":"%s","flaky_good":%s,"flaky_bad":%s,"bisect_rc":%s}\n' \
  "$WITNESS" "$good" "$first_bad" "$pr" "$mech" "$flaky_good" "$flaky_bad" "$bisect_rc"
