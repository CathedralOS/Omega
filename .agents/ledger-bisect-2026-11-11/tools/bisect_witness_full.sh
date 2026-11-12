#!/bin/bash
# git bisect run helper: 0 good, 1 bad (PIN regression), 125 skip/build/other
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${BISECT_REPO_ROOT:-$(git rev-parse --show-toplevel)}"
export BISECT_REPO_ROOT="$ROOT"
cd "$ROOT"
# Ledger-only merge commits cannot change compiler behavior.
if git rev-parse -q --verify HEAD^1 >/dev/null 2>&1; then
  mapfile -t files < <(git diff --name-only HEAD^1 HEAD)
  if ((${#files[@]} > 0)); then
    ledger_only=1
    for f in "${files[@]}"; do
      if [[ "$f" != tests/omega/corpus_outcomes.txt ]] && [[ ! "$f" =~ \.md$ ]]; then
        ledger_only=0
        break
      fi
    done
    if ((ledger_only)); then
      exit 125
    fi
  fi
fi
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
SHA="$(git rev-parse HEAD)"
TD="$ROOT/target/verify-$SHA"
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TD"
mkdir -p "$TD"
nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$TD"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "${BISECT_TOOLCACHE:-$(dirname "$0)")}/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt
