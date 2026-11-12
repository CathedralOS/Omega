#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
TOOLS="$(dirname "$0")"
# Skip ledger-only commits on first-parent edge.
if parent=$(git rev-parse --verify HEAD^1 2>/dev/null); then
  mapfile -t touched < <(git diff --name-only "$parent" HEAD || true)
  if ((${#touched[@]} > 0)); then
  only_docs=true
    for f in "${touched[@]}"; do
      case "$f" in
        tests/omega/corpus_outcomes.txt | *.md) ;;
        *) only_docs=false; break ;;
      esac
    done
    $only_docs && exit 125
  fi
fi
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
TD="${CARGO_TARGET_DIR:-$ROOT/target-pin}"
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TD" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(bash "$(dirname "$0")/get_corpus_runner.sh")
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$TOOLS/bisect_test_pin.py" "$WITNESS" /tmp/bisect_one_record.txt
