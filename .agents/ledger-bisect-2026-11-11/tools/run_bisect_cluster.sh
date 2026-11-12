#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
TOOL="$(cd "$(dirname "$0")" && pwd)"
ROOT="${BISECT_ROOT:-$(git -C "$TOOL" rev-parse --show-toplevel)}"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
export LEDGER_PIN_FILE=/tmp/ledger-bisect-pin/corpus_outcomes.txt

witness_rc() {
  "$TOOL/bisect_witness.sh"
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
  if is_ledger_only_commit "$sha"; then
    echo "ledger-only commit; skipped"
    return
  fi
  git log -1 --format=%s "$sha"
}

git checkout -q "$PIN"
set +e
witness_rc >/dev/null
pin_rc=$?
set -e
[[ "$pin_rc" -eq 1 ]] || { echo "{\"error\":\"PIN not bad under frozen baseline\",\"rc\":$pin_rc}"; exit 2; }

good=""
dist=1
while [[ "$dist" -le 20000 ]]; do
  cand=$(git rev-parse --verify "${PIN}~${dist}" 2>/dev/null) || break
  git checkout -q "$cand"
  set +e
  rc=$(witness_rc)
  set -e
  if [[ "$rc" -eq 0 ]]; then
    good="$cand"
    break
  fi
  dist=$((dist * 2))
done
git checkout -q "$PIN"
[[ -n "$good" ]] || { echo '{"error":"no good first-parent ancestor"}'; exit 3; }

# Stability at ends (deterministic vs flaky)
flaky_good=0
flaky_bad=0
for _ in 1 2 3; do
  git checkout -q "$good"
  set +e; witness_rc >/dev/null; r=$?; set -e
  [[ "$r" -ne 0 ]] && flaky_good=1
done
for _ in 1 2 3; do
  git checkout -q "$PIN"
  set +e; witness_rc >/dev/null; r=$?; set -e
  [[ "$r" -ne 1 ]] && flaky_bad=1
done
git checkout -q "$PIN"

git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$good"
export BISECT_WITNESS="$WITNESS"
: >/tmp/bisect_run.log
set +e
git bisect run "$TOOL/bisect_witness.sh" 2>&1 | tee /tmp/bisect_run.log
bisect_rc=$?
set -e
first_bad=$(git bisect log | sed -n 's/^first bad commit: \[\([0-9a-f]*\)\].*/\1/p' | head -1)
[[ -z "$first_bad" ]] && first_bad=$(rg -m1 'first bad commit: \[([0-9a-f]+)\]' -o -r '$1' /tmp/bisect_run.log || true)
# Advance past ledger-only first bad if bisect landed there.
if [[ -n "$first_bad" ]] && is_ledger_only_commit "$first_bad"; then
  walk="$first_bad"
  for _ in $(seq 1 50); do
    walk=$(git rev-parse --verify "${walk}^@" 2>/dev/null) || break
    if ! is_ledger_only_commit "$walk"; then
      set +e
      git checkout -q "$walk"
      rc=$(witness_rc)
      git checkout -q "$PIN"
      set -e
      [[ "$rc" -eq 1 ]] && first_bad="$walk" && break
    fi
  done
fi
git bisect reset "$PIN" >/dev/null

subj=$(git log -1 --format=%s "$first_bad" 2>/dev/null || echo unknown)
pr=$(echo "$subj" | sed -n 's/.*(#\([0-9][0-9]*\)).*/\1/p')
mech=$(mechanism_for "$first_bad")

printf '{"witness":"%s","good":"%s","first_bad":"%s","pr":"%s","mechanism":"%s","flaky_good":%s,"flaky_bad":%s,"bisect_rc":%s}\n' \
  "$WITNESS" "$good" "$first_bad" "$pr" "$mech" "$flaky_good" "$flaky_bad" "$bisect_rc"
