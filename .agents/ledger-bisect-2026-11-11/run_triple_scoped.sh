#!/bin/bash
# Triple witness at commit with isolated CARGO_TARGET_DIR. Args: commit witness [label]
set -euo pipefail
COMMIT="${1:?}"
WITNESS="${2:?}"
LABEL="${3:-$COMMIT}"
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
if [[ ! -d "$ROOT/.git" ]]; then
  ROOT="$(git -C "$ROOT" rev-parse --show-toplevel)"
fi
export OMEGA_REPO="$ROOT"
SCOPE="${COMMIT:0:12}"
export CARGO_TARGET_DIR="/tmp/ledger-bisect-target-$SCOPE"
cd "$ROOT"
git checkout -q -f "$COMMIT"
SETUP="/tmp/ledger-bisect-pin/tools/setup_pin_specs.py"
[[ -f "$SETUP" ]] || SETUP="$(dirname "$0")/setup_pin_specs.py"
python3 "$SETUP" >/dev/null
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1
CARGO_PROFILE_DEV_OPT_LEVEL=1 nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1
RUNNER=$(ls -t "$CARGO_TARGET_DIR/debug/build/compiler"/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || { echo "  build exit=125 (no runner)"; exit 0; }
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
echo "=== $LABEL commit=$COMMIT witness=$WITNESS target=$CARGO_TARGET_DIR ==="
for i in 1 2 3; do
  export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/triple_scoped_record.txt
  rm -f /tmp/triple_scoped_record.txt
  nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  set +e
  python3 /tmp/ledger-bisect-pin/tools/bisect_test.py "$WITNESS" /tmp/triple_scoped_record.txt
  rc=$?
  set -e
  status=$(grep -m1 "^$WITNESS " /tmp/triple_scoped_record.txt || echo "$WITNESS MISSING")
  echo "  run$i exit=$rc ($status)"
done
