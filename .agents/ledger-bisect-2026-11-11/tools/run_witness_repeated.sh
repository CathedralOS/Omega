#!/bin/bash
# Usage: run_witness_repeated.sh <commit> <witness> [runs=3]
set -euo pipefail
COMMIT="${1:?}"
WITNESS="${2:?}"
RUNS="${3:-3}"
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
cd "$ROOT"
git checkout -q "$COMMIT"
FULL=$(git rev-parse HEAD)
export BISECT_WITNESS="$WITNESS"
TOOL="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
echo "commit=$FULL"
echo "subject=$(git log -1 --format=%s)"
unset CARGO_TARGET_DIR
BISECT_TARGET="/tmp/omega-target-bisect-$(git rev-parse HEAD)"
mkdir -p "$BISECT_TARGET"
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || { echo "build failed"; exit 125; }
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx test -p compiler --test corpus_runner --no-run < /dev/null >/dev/null 2>&1 || { echo "runner build failed"; exit 125; }
export BISECT_SKIP_BUILD=1
for ((i=1; i<=RUNS; i++)); do
  rm -f /tmp/bisect_one_record.txt
  rc=0
  "$TOOL" || rc=$?
  status=""
  diag=""
  if [[ -f /tmp/bisect_one_record.txt ]]; then
    status=$(head -1 /tmp/bisect_one_record.txt | awk '{print $2}')
    diag=$(grep -m1 '^	' /tmp/bisect_one_record.txt | sed 's/^\t//' || true)
  fi
  echo "run$i rc=$rc status=${status:-none} diag=${diag:-none}"
done
