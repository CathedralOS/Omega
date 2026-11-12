#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
chmod -R u+w "$ROOT/target" 2>/dev/null || true

# Skip ledger-only first-parent commits.
if git rev-parse -q --verify HEAD^1 >/dev/null 2>&1; then
  mapfile -t CHANGED < <(git diff --name-only HEAD^1 HEAD)
  if ((${#CHANGED[@]} > 0)); then
  only_ledger=1
  for f in "${CHANGED[@]}"; do
    case "$f" in
      tests/omega/corpus_outcomes.txt) ;;
      *.md) ;;
      *)
        only_ledger=0
        break
        ;;
    esac
  done
  if ((only_ledger)); then
    exit 125
  fi
  fi
fi

rm -f /tmp/bisect_one_record.txt
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test -p compiler corpus_runner --no-run < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(
  find "$ROOT/target/debug/build/compiler" -path '*/out/corpus_runner-*' -type f ! -name '*.d' \
    -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -d' ' -f2-
)
[[ -n "${RUNNER:-}" && -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$(dirname "$0")/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt
