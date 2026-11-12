#!/bin/bash
# first-parent git bisect for one witness; prints JSON to stdout.
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
TOOL="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$TOOL" rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"

witness_rc() {
  set +e
  "$TOOL/bisect_witness.sh" >/dev/null
  local rc=$?
  set -e
  echo "$rc"
}

stability_check() {
  local sha=$1 label=$2
  git checkout -q "$sha"
  local a b c
  a=$(witness_rc)
  b=$(witness_rc)
  c=$(witness_rc)
  git checkout -q "$PIN"
  echo "${label}_codes=${a},${b},${c}" >&2
  if [[ "$a" != "$b" || "$b" != "$c" ]]; then
    echo "flaky_${label}=1" >&2
    return 1
  fi
  echo "$a"
}

find_good_commit() {
  local candidates=()
  mapfile -t c1 < <(git log --first-parent --format=%H -S "$WITNESS checked" -- tests/omega/corpus_outcomes.txt | head -15)
  candidates+=("${c1[@]}")
  mapfile -t c2 < <(git log --first-parent --format=%H -S "$WITNESS built" -- tests/omega/corpus_outcomes.txt | head -10)
  candidates+=("${c2[@]}")
  local stamp
  stamp=$(git show "$PIN:tests/omega/corpus_outcomes.txt" | rg "^$WITNESS " | rg -o '@[0-9a-f]+' | head -1 | tr -d '@' || true)
  [[ -n "$stamp" ]] && candidates+=("$stamp")
  local sha full rc
  for sha in "${candidates[@]}"; do
    [[ -z "$sha" ]] && continue
    full=$(git rev-parse --verify "${sha}^{commit}" 2>/dev/null) || continue
    git checkout -q "$full"
    rc=$(witness_rc)
    if [[ "$rc" -eq 0 ]]; then
      git checkout -q "$PIN"
      echo "$full"
      return 0
    fi
  done

  # Exponential first-parent search backward from PIN for exit 0.
  local dist=1
  local good=""
  while [[ "$dist" -le 20000 ]]; do
    local cand
    cand=$(git rev-parse "${PIN}~${dist}" 2>/dev/null) || break
    git checkout -q "$cand"
    rc=$(witness_rc)
    if [[ "$rc" -eq 0 ]]; then
      good="$cand"
      break
    fi
    dist=$((dist * 2))
  done
  git checkout -q "$PIN"
  [[ -n "$good" ]] || return 1
  echo "$good"
}

git checkout -q "$PIN"
bad_rc=$(witness_rc)
[[ "$bad_rc" -eq 1 ]] || { echo "{\"error\":\"PIN not bad rc=$bad_rc\"}"; exit 2; }

good=$(find_good_commit) || { echo '{"error":"no good commit found"}'; exit 3; }
good_rc=$(stability_check "$good" good) || FLAKY_GOOD=1
pin_rc=$(stability_check "$PIN" bad) || FLAKY_BAD=1
[[ "${FLAKY_GOOD:-0}" != 1 && "$good_rc" != "0" ]] && { echo "{\"error\":\"good not passing rc=$good_rc\"}"; exit 4; }

git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$good"
export BISECT_WITNESS="$WITNESS"
set +e
git bisect run "$TOOL/bisect_witness.sh" >/tmp/bisect_run.log 2>&1
bisect_rc=$?
set -e
first_bad=$(sed -n 's/^first bad commit: \[\([0-9a-f]*\)\].*/\1/p' /tmp/bisect_run.log | head -1)
if [[ -z "$first_bad" ]]; then
  first_bad=$(rg '^Bisecting: .*$' /tmp/bisect_run.log | tail -1 || true)
fi
git bisect reset "$PIN" >/dev/null

[[ -n "$first_bad" && "$first_bad" != "$PIN" ]] || first_bad=$(git rev-parse "${first_bad}^{commit}" 2>/dev/null || echo "$first_bad")
subj=$(git log -1 --format=%s "$first_bad" 2>/dev/null || echo unknown)
pr=$(echo "$subj" | sed -n 's/.*(#\([0-9][0-9]*\)).*/\1/p')
mech=$(git show --stat --format= "$first_bad" 2>/dev/null | head -12 | tr '\n' ' ' | cut -c1-220)

printf '{"witness":"%s","good":"%s","first_bad":"%s","pr":"%s","mechanism":"%s","flaky_good":%s,"flaky_bad":%s,"bisect_rc":%s}\n' \
  "$WITNESS" "$good" "$first_bad" "$pr" "$mech" "${FLAKY_GOOD:-0}" "${FLAKY_BAD:-0}" "$bisect_rc"
