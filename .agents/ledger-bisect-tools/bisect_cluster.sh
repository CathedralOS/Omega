#!/bin/bash
# Usage: bisect_cluster.sh <cluster_id> <witness> <out_json>
set -euo pipefail
CLUSTER_ID="${1:?id}"
WITNESS="${2:?witness}"
OUT="${3:?out json}"
PIN=51e8c45c793007f211d0d9baee7de16767172444
TOOLROOT=/tmp/ledger-bisect-tools
ROOT="$(git -C "$TOOLROOT" rev-parse --show-toplevel 2>/dev/null || git rev-parse --show-toplevel)"
cd "$ROOT"
SCRIPT="$TOOLROOT/bisect_witness.sh"
MEASURE="$TOOLROOT/measure_one.sh"
TEST="$TOOLROOT/bisect_test.py"
BRANCH="${BISECT_RETURN_BRANCH:-work/ledger-bisect-part-29}"

ledger_only_commit() {
  git rev-parse HEAD^1 >/dev/null 2>&1 || return 1
  mapfile -t touched < <(git diff --name-only HEAD^1 HEAD 2>/dev/null || true)
  [[ ${#touched[@]} -eq 0 ]] && return 1
  for f in "${touched[@]}"; do
    case "$f" in tests/omega/corpus_outcomes.txt|*.md) ;; *) return 1 ;; esac
  done
  return 0
}

test_witness() {
  bash "$MEASURE" "$WITNESS" >/dev/null 2>&1 || return 125
  python3 "$TEST" "$WITNESS" "/tmp/measure_${WITNESS//\//_}.txt"
}

eval_at() {
  local ref=$1
  git checkout "$ref" -q
  if ledger_only_commit; then
    echo 125
    return 0
  fi
  set +e
  test_witness >/dev/null 2>&1
  local rc=$?
  set -e
  echo "$rc"
}

find_good() {
  # First-parent chain from PIN; binary-search the newest commit that still passes.
  local -a chain=("$PIN")
  local cur=$PIN i
  for i in $(seq 1 2500); do
    local parent
    parent=$(git rev-parse "${cur}^1" 2>/dev/null || true)
    [[ -z "$parent" || "$parent" == "$cur" ]] && break
    chain+=("$parent")
    cur=$parent
  done
  local lo=0 hi=$((${#chain[@]} - 1))
  while [[ "$lo" -lt "$hi" ]]; do
    local mid=$(( (lo + hi) / 2 ))
    local rc
    rc=$(eval_at "${chain[$mid]}")
    if [[ "$rc" == "125" ]]; then
      # Treat unknown ledger-only as bad for narrowing; walk up on first-parent.
      lo=$((mid + 1))
      continue
    fi
    if [[ "$rc" == "0" ]]; then
      hi=$mid
    else
      lo=$((mid + 1))
    fi
  done
  local final_rc
  final_rc=$(eval_at "${chain[$lo]}")
  [[ "$final_rc" == "0" ]] || return 1
  echo "${chain[$lo]}"
  return 0
}

export BISECT_WITNESS="$WITNESS"
git checkout "$BRANCH" -q 2>/dev/null || git checkout "$PIN" -q

if [[ -n "${BISECT_GOOD:-}" ]]; then
  GOOD="$BISECT_GOOD"
else
  GOOD=$(find_good) || {
  git checkout "$BRANCH" -q 2>/dev/null || git checkout "$PIN" -q
  python3 - "$OUT" <<PY
import json
json.dump({"cluster_id":"$CLUSTER_ID","witness":"$WITNESS","status":"propagated","reason":"no_good_on_first_parent"}, open("$OUT","w"), indent=2)
PY
  exit 0
  }
fi

git bisect reset 2>/dev/null || true
if ! git bisect start --first-parent "$PIN" "$GOOD" 2>&1 | tee /tmp/bisect_${CLUSTER_ID}.log; then
  git checkout "$BRANCH" -q 2>/dev/null || true
  python3 - "$OUT" <<PY
import json
json.dump({"cluster_id":"$CLUSTER_ID","witness":"$WITNESS","good":"$GOOD","status":"propagated","reason":"bisect_start_failed"}, open("$OUT","w"), indent=2)
PY
  exit 0
fi

set +e
git bisect run "$SCRIPT" 2>&1 | tee -a /tmp/bisect_${CLUSTER_ID}.log
BI_RC=$?
set -e
BREAK=$(git rev-parse HEAD)
git bisect reset 2>/dev/null || true
git checkout "$BRANCH" -q 2>/dev/null || git checkout "$PIN" -q

STATUS=bisected
REASON=""
if [[ "$BREAK" == "$PIN" ]]; then
  STATUS=propagated
  REASON="first_bad_equals_pin"
fi
mapfile -t touched < <(git diff --name-only "${BREAK}^1" "$BREAK" 2>/dev/null || true)
ledger_only=1
for f in "${touched[@]}"; do
  case "$f" in tests/omega/corpus_outcomes.txt|*.md) ;; *) ledger_only=0 ;; esac
done
if [[ "$ledger_only" -eq 1 && ${#touched[@]} -gt 0 ]]; then
  STATUS=propagated
  REASON="ledger_only_break"
fi

SUBJ=$(git log -1 --format='%s' "$BREAK" 2>/dev/null || true)
PR=$(sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' <<<"$SUBJ" | head -1)
DIFF=$(git show --stat --format= "$BREAK" 2>/dev/null | head -12 | tr '\n' '; ')

python3 - "$OUT" <<PY
import json, subprocess
mech = """$SUBJ"""
touched = """${touched[*]}""".split()
if touched and "$STATUS" == "bisected":
    show = subprocess.run(["git","show","--no-color","-U1","$BREAK","--",*[f for f in touched if not f.endswith('.md') and f != 'tests/omega/corpus_outcomes.txt'][:3]], capture_output=True, text=True).stdout
    mech_line = show.strip().split("\\n")[:6]
    if mech_line:
        mech = ("""$SUBJ""" + " | " + " ".join(mech_line))[:400]
json.dump({
  "cluster_id": "$CLUSTER_ID",
  "witness": "$WITNESS",
  "good": "$GOOD",
  "pin": "$PIN",
  "break": "$BREAK",
  "pr": "$PR",
  "subject": """$SUBJ""",
  "mechanism": mech,
  "diff_stat": """$DIFF""",
  "status": "$STATUS",
  "reason": "$REASON",
  "bisect_rc": $BI_RC,
  "touched": touched,
}, open("$OUT", "w"), indent=2)
PY
