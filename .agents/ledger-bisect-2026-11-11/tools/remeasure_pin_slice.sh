#!/bin/bash
# Re-measure part-9 slice fixtures at PIN with commit-scoped CARGO_TARGET_DIR.
set -euo pipefail
ROOT="${REPO_ROOT:-/agent/repos/omega}"
PIN="${PIN:-51e8c45c793007f211d0d9baee7de16767172444}"
AGENT="$ROOT/.agents/ledger-bisect-2026-11-11"
TOOLS="$AGENT/tools"
WORK="$AGENT/work"
SLICE_JSON="$AGENT/part9_slice.json"
OBS="$WORK/pin_observed.txt"
STABILITY="$WORK/pin_remeasure_stability.txt"
CLASSIFY_JSON="$WORK/pin_classify.json"
SLICE_LIST="$WORK/slice_24_fixtures.txt"
PREPARE="$TOOLS/prepare_bisect_pin.sh"

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export BISECT_PIN_LEDGER=/tmp/ledger_bisect_corpus_outcomes.txt

python3 - "$SLICE_JSON" "$SLICE_LIST" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
seen, fixtures = set(), []
for cluster in data["slice"]:
    for f in cluster["members"]:
        if f not in seen:
            seen.add(f)
            fixtures.append(f)
assert len(fixtures) == 24, len(fixtures)
open(sys.argv[2], "w").write("\n".join(fixtures) + "\n")
print("fixtures", len(fixtures))
PY

WT="/tmp/ledger-bisect-wt-${PIN:0:12}"
if ! git -C "$WT" rev-parse HEAD >/dev/null 2>&1; then
  git -C "$ROOT" worktree add --detach "$WT" "$PIN" >/dev/null
fi
cp "$WT/tests/omega/corpus_outcomes.txt" /tmp/ledger_bisect_corpus_outcomes.txt

export REPO_ROOT="$WT" OMEGA_REPO="$WT"
CARGO_TARGET_DIR="$WT/target/bisect-pin-measure-${PIN:0:12}"
mkdir -p "$CARGO_TARGET_DIR"
chmod -R u+w "$CARGO_TARGET_DIR" 2>/dev/null || true
cd "$WT"

echo "building at PIN in $CARGO_TARGET_DIR ..."
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$CARGO_TARGET_DIR" nice -n 19 mbx build -p omega </dev/null
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$CARGO_TARGET_DIR" nice -n 19 mbx test --no-run -p compiler --test corpus_runner </dev/null
RUNNER=$(ls -t "$CARGO_TARGET_DIR"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | grep -v '\.d$' | head -1)
[[ -n "$RUNNER" && -x "$RUNNER" ]] || { echo "no corpus_runner"; exit 1; }

export OMEGA_CORPUS_ROOT="$WT" OMEGA_LIBRARY_ROOT="$WT/source/library"

: >"$OBS"
: >"$STABILITY"

mapfile -t FIXTURES <"$SLICE_LIST"
for fixture in "${FIXTURES[@]}"; do
  statuses=()
  diags=()
  for run in 1 2 3; do
    export OMEGA_CORPUS_FIXTURE_FILTER="$fixture"
    export OMEGA_CORPUS_RECORDS=/tmp/pin_single_record.txt
    rm -f /tmp/pin_single_record.txt
    nice -n 19 "$RUNNER" </dev/null >/dev/null 2>&1 || true
    rec=$(cat /tmp/pin_single_record.txt 2>/dev/null || true)
    line=$(printf '%s\n' "$rec" | awk '/^pass\// || /^fail\// || /^run\// {print; exit}')
  [[ -z "$line" ]] && line="MISSING"
    statuses+=("$line")
    if [[ "$run" -eq 1 ]]; then
      canon="$rec"
    elif [[ "$rec" != "$canon" ]]; then
      echo "STABILITY_MISMATCH $fixture run$run" >>"$STABILITY"
    fi
    # bisect exit vs ledger (pin json not required for meets-ledger)
    if [[ "$run" -eq 1 && -f "$PREPARE" ]]; then
      :
    fi
  done
  # append canonical record from run 1 (all runs must match)
  if [[ -n "${canon:-}" ]]; then
    printf '%s\n' "$canon" >>"$OBS"
  fi
  echo "pin-remeasure commit=$PIN fixture=$fixture run1=${statuses[0]} run2=${statuses[1]} run3=${statuses[2]}" >>"$STABILITY"
done

mkdir -p /tmp/ledger-bisect-tools
cp -f "$TOOLS/bisect_test.py" /tmp/ledger-bisect-tools/
for fixture in "${FIXTURES[@]}"; do
  "$PREPARE" "$fixture" "$PIN" "$OBS" >/dev/null
  for run in 1 2 3; do
    export OMEGA_CORPUS_FIXTURE_FILTER="$fixture"
    export OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
    rm -f /tmp/bisect_one_record.txt
    nice -n 19 "$RUNNER" </dev/null >/dev/null 2>&1 || true
    code=$(python3 /tmp/ledger-bisect-tools/bisect_test.py "$fixture" /tmp/bisect_one_record.txt || echo 125)
    echo "bisect-exit commit=$PIN fixture=$fixture run$run exit=$code" >>"$STABILITY"
  done
done

python3 "$TOOLS/classify_corpus.py" "$OBS" "$SLICE_LIST" "$CLASSIFY_JSON"
echo "wrote $OBS"
python3 - "$CLASSIFY_JSON" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))["counts"])
PY
