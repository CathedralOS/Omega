#!/bin/bash
# Exponential first-parent walk from the PIN, then git bisect run --first-parent.
# One witness. Members are rechecked at the edges and listed for splits.
set -uo pipefail
WITNESS="${1:?witness}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
shift 2
MEMBERS=("$@")
if ((${#MEMBERS[@]} == 0)); then
  MEMBERS=("$WITNESS")
fi
TOOL="$(cd "$(dirname "$0")" && pwd)"
ROOT="${BISECT_ROOT:-/tmp/bisect-wt}"
SLUG="${WITNESS//\//__}"
OUT="$TOOL/../runs/$SLUG"
mkdir -p "$OUT"
RCLOG="$OUT/rc.log"
: >"$RCLOG"
export BISECT_ROOT="$ROOT" BISECT_WITNESS="$WITNESS" BISECT_RC_LOG="$RCLOG"
cd "$ROOT"
git bisect reset >/dev/null 2>&1 || true
git checkout -q "$PIN"

intro=$(git log --diff-filter=A --format=%H -1 -- "tests/omega/$WITNESS" || true)
echo "$intro" >"$OUT/fixture-add.txt"

run_both() {
  local sha rc
  git checkout -q "$1"
  sha=$(git rev-parse HEAD)
  export BISECT_MODE=both
  set +e
  "$TOOL/bisect_witness.sh"
  rc=$?
  set -e
  if [[ "$rc" -eq 125 ]]; then
    echo 125 125
    return
  fi
  grep "^${sha} both stop=" "$RCLOG" | tail -1 | sed -n 's/.*stop=\([0-9]*\) class=\([0-9]*\).*/\1 \2/p'
}

read -r pin_stop pin_class < <(run_both "$PIN")
echo "pin $PIN $pin_stop $pin_class" | tee "$OUT/pin.txt"
if [[ "$pin_stop" != "1" || "$pin_class" != "1" ]]; then
  echo "PIN is not bad for both witnesses: stop=$pin_stop class=$pin_class" | tee "$OUT/ERROR"
  exit 2
fi

echo "dist sha stop class" >"$OUT/walk.txt"
stop_good=""
class_good=""
dist=1
while true; do
  cand=$(git rev-parse --verify "${PIN}~${dist}" 2>/dev/null) || break
  read -r stop class < <(run_both "$cand")
  echo "$dist $cand $stop $class" | tee -a "$OUT/walk.txt"
  [[ -z "$stop_good" && "$stop" == "0" ]] && stop_good=$cand
  [[ -z "$class_good" && "$class" == "0" ]] && class_good=$cand
  if [[ -n "$stop_good" && -n "$class_good" ]]; then
    break
  fi
  if [[ -n "$intro" && "$cand" == "$intro" ]]; then
    break
  fi
  if [[ -n "$intro" ]] && git merge-base --is-ancestor "$cand" "$intro" >/dev/null 2>&1; then
    break
  fi
  dist=$((dist * 2))
  [[ "$dist" -gt 200000 ]] && break
done

if [[ -n "$intro" ]] && ! grep -q "$intro" "$OUT/walk.txt"; then
  read -r stop class < <(run_both "$intro")
  echo "fixture-add $intro $stop $class" | tee -a "$OUT/walk.txt"
  [[ -z "$stop_good" && "$stop" == "0" ]] && stop_good=$intro
  [[ -z "$class_good" && "$class" == "0" ]] && class_good=$intro
fi

echo "$stop_good" >"$OUT/good-stop.txt"
echo "$class_good" >"$OUT/good-class.txt"
if [[ -z "$stop_good" ]]; then
  echo "never-good since ${intro:-unknown}" >"$OUT/never-good-stop.txt"
fi
if [[ -z "$class_good" ]]; then
  echo "never-good since ${intro:-unknown}" >"$OUT/never-good-class.txt"
fi

bisect_mode() {
  local mode=$1 good=$2
  [[ -n "$good" ]] || return 0
  git bisect reset >/dev/null 2>&1 || true
  git checkout -q "$PIN"
  export BISECT_MODE="$mode" BISECT_WITNESS="$WITNESS"
  git bisect start --first-parent "$PIN" "$good"
  set +e
  git bisect run "$TOOL/bisect_witness.sh" >"$OUT/bisect-${mode}.log" 2>&1
  set -e
  git bisect log >"$OUT/bisect-${mode}.gitlog" 2>/dev/null || true
  local first
  first=$(grep -oE '^[0-9a-f]{40} is the first bad commit' "$OUT/bisect-${mode}.log" | awk '{print $1}' | head -1)
  if [[ -z "$first" ]]; then
    first=$(sed -n 's/^# first bad commit: \[\([0-9a-f]\{40\}\)\] .*/\1/p' "$OUT/bisect-${mode}.gitlog" | head -1)
  fi
  git bisect reset >/dev/null 2>&1 || true
  git checkout -q "$PIN"
  if [[ -z "$first" ]]; then
    echo "no-first-bad" >"$OUT/first-${mode}.txt"
    return 0
  fi
  git rev-parse "$first" >"$OUT/first-${mode}.txt"
  git rev-parse "${first}^" >"$OUT/parent-${mode}.txt"
  git log -1 --format=%s "$first" >"$OUT/subject-${mode}.txt"
  git show --format=fuller --unified=6 "$first" >"$OUT/show-${mode}.diff"
}

bisect_mode stop "$stop_good"
bisect_mode class "$class_good"

triple() {
  local mode=$1 sha=$2 label=$3
  local i rc
  : >"$OUT/triple-${mode}-${label}.txt"
  export BISECT_MODE="$mode" BISECT_WITNESS="$WITNESS"
  for i in 1 2 3; do
    git checkout -q "$sha"
    set +e
    "$TOOL/bisect_witness.sh"
    rc=$?
    set -e
    echo "$rc" | tee -a "$OUT/triple-${mode}-${label}.txt"
  done
}

for mode in stop class; do
  first=$(cat "$OUT/first-${mode}.txt" 2>/dev/null || true)
  [[ -n "$first" && "$first" != "no-first-bad" ]] || continue
  parent=$(cat "$OUT/parent-${mode}.txt")
  triple "$mode" "$first" bad
  triple "$mode" "$parent" good
  : >"$OUT/members-${mode}.txt"
  for member in "${MEMBERS[@]}"; do
    export BISECT_WITNESS="$member" BISECT_MODE="$mode"
    git checkout -q "$parent"
    set +e
    "$TOOL/bisect_witness.sh"
    prc=$?
    git checkout -q "$first"
    "$TOOL/bisect_witness.sh"
    brc=$?
    set -e
    echo "$member parent=$prc bad=$brc" | tee -a "$OUT/members-${mode}.txt"
  done
done

python3 "$TOOL/count_125.py" "$RCLOG" "$OUT" >"$OUT/skip125.txt"
echo "done $WITNESS" | tee "$OUT/DONE"
