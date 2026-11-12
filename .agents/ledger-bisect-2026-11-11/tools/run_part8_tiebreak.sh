#!/bin/bash
# Tie-break: fresh CARGO_TARGET_DIR per commit; fixed-PIN bisect_test.
set -euo pipefail
ROOT=/agent/repos/omega
STABLE_TOOLS=/tmp/ledger-bisect-tools
mkdir -p "$STABLE_TOOLS"
cp "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" \
  "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_test.py" \
  "$ROOT/.agents/ledger-bisect-2026-11-11/tools/pin_bisect_setup.py" \
  "$STABLE_TOOLS/"
TOOLS="$STABLE_TOOLS"
PIN_LEDGER=/tmp/pin_corpus_outcomes_51e8c45.txt
FIX_DIR=/tmp/ledger-bisect-part-8/opaque
RECORDS=/tmp/part8_pin_records.txt
export OMEGA_REPO="$ROOT" PIN_LEDGER="$PIN_LEDGER" BISECT_REPO_ROOT="$ROOT"

cd "$ROOT"
git checkout -qf work/ledger-bisect-part-8
cp "$ROOT/tests/omega/corpus_outcomes.txt" "$PIN_LEDGER"

MEMBERS=(
  pass/facts/opaque_independent_qualified_result_import_alias_exit
  pass/facts/opaque_independent_qualified_result_renumbered_catalog_exit
  pass/facts/opaque_independent_qualified_result_through_local_exit
)

run_one() {
  local commit=$1 witness=$2 rep=$3
  export BISECT_WITNESS="$witness" BISECT_FIXTURE_DIR="$FIX_DIR"
  export BISECT_CARGO_TARGET_DIR="/tmp/bisect-target-${commit}"
  python3 "$TOOLS/pin_bisect_setup.py" "$witness" "$RECORDS" "$FIX_DIR" >/dev/null
  git checkout -qf "$commit"
  # shellcheck disable=SC2086
  set +e
  export BISECT_TEST_SCRIPT="$TOOLS/bisect_test.py"
  bash "$TOOLS/bisect_witness.sh" >/dev/null 2>&1
  local rc=$?
  set -e
  local head_sha
  head_sha=$(git rev-parse HEAD)
  local rec="/tmp/bisect_one_record_${head_sha}.txt"
  local st="(no_record)"
  [[ -f "$rec" ]] && st=$(head -1 "$rec" | awk '{print $2}')
  local diag=""
  [[ -f "$rec" ]] && diag=$(sed -n '2p' "$rec" | head -c 120)
  echo "commit=$commit rep=$rep fixture=${witness##*/} exit=$rc status=$st diag=${diag}"
}

for commit in \
  4d6278c81b8360d8100cef23e2100be18ddf32a7 \
  60cb5e1f79dd05465f77775990e46bcc2e789470; do
  rm -rf "/tmp/bisect-target-${commit}"
  mkdir -p "/tmp/bisect-target-${commit}"
  for witness in "${MEMBERS[@]}"; do
    for rep in 1 2 3; do
      run_one "$commit" "$witness" "$rep"
    done
  done
done

commit=60ee163d4f0f5f8b9349ecf253660a917917a7c7
rm -rf "/tmp/bisect-target-${commit}"
mkdir -p "/tmp/bisect-target-${commit}"
for witness in "${MEMBERS[@]}"; do
  run_one "$commit" "$witness" 1
done

git checkout -qf 51e8c45c793007f211d0d9baee7de16767172444
