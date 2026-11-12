#!/bin/bash
# Walk first-parent ancestors from PIN until witness bisect test returns 0.
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${2:?pin commit}"
ROOT="${3:-$(git rev-parse --show-toplevel)}"
export BISECT_REPO_ROOT="$ROOT"
export BISECT_PIN_LEDGER="${BISECT_PIN_LEDGER:?}"
export BISECT_WITNESS_PROFILE="${BISECT_WITNESS_PROFILE:?}"
export BISECT_WITNESS="$WITNESS"
TOOLS="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

run_one() {
  local sha="$1"
  git checkout "$sha" --quiet
  if "$TOOLS/bisect_witness.sh" >/dev/null 2>&1; then
  "$TOOLS/bisect_witness.sh"
  else
    echo 125
  fi
}

git checkout "$PIN" --quiet
cur="$PIN"
steps=0
max="${BISECT_FIND_GOOD_MAX:-400}"
while ((steps < max)); do
  code=125
  if out=$("$TOOLS/bisect_witness.sh" 2>/dev/null); then
    code=$?
  else
    code=$?
  fi
  # bisect_witness.sh exit code is the classification
  if "$TOOLS/bisect_witness.sh" >/dev/null 2>&1; then
    code=0
  elif [[ $? -eq 1 ]]; then code=1; else code=125; fi
  # Simpler: call python after runner inline below
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" mbx build -p omega < /dev/null >/dev/null 2>&1 || { code=125; echo "$cur skip_build"; parent=$(git rev-parse HEAD^1 2>/dev/null || true); [[ -n "$parent" ]] || break; git checkout "$parent" --quiet; cur="$parent"; steps=$((steps+1)); continue; }
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" mbx test -p compiler --no-run --test corpus_runner < /dev/null >/dev/null 2>&1 || { code=125; parent=$(git rev-parse HEAD^1 2>/dev/null || true); [[ -n "$parent" ]] || break; git checkout "$parent" --quiet; cur="$parent"; steps=$((steps+1)); continue; }
  RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
  export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/find_good_record.txt
  rm -f /tmp/find_good_record.txt
  nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  code=$(python3 "$TOOLS/bisect_test.py" "$WITNESS" /tmp/find_good_record.txt || echo 125)
  if [[ "$code" == "0" ]]; then
    echo "GOOD $cur ($steps steps from PIN)"
    exit 0
  fi
  parent=$(git rev-parse HEAD^1 2>/dev/null || true)
  [[ -n "$parent" ]] || break
  git checkout "$parent" --quiet
  cur="$parent"
  steps=$((steps+1))
done
echo "GOOD_NOT_FOUND after $steps steps (last $cur)" >&2
exit 1
