#!/bin/bash
# Run corpus filter at $COMMIT for space-separated fixtures in $MEMBERS; print status lines.
set -euo pipefail
COMMIT="${1:?commit}"
shift
MEMBERS=("$@")
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git checkout -q "$COMMIT"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" mbx build -p omega < /dev/null >/dev/null 2>&1
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
FILTER=$(IFS=,; echo "${MEMBERS[*]}")
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS=/tmp/members_record.txt
rm -f /tmp/members_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
for m in "${MEMBERS[@]}"; do
  line=$(grep -m1 "^$m " /tmp/members_record.txt || echo "$m MISSING")
  echo "$line"
done
