#!/bin/bash
# git bisect run: 0=PIN-expected outcome, 1=PIN-broken reproduction, 125=skip
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
TOOLS="$(dirname "$0")"
TDIR="${CARGO_TARGET_DIR:-/tmp/omega-bisect-target}"
mapfile -t touched < <(git diff --name-only HEAD^1 HEAD 2>/dev/null || true)
if ((${#touched[@]} > 0)); then
  compiler=0
  for p in "${touched[@]}"; do
    [[ "$p" == tests/omega/corpus_outcomes.txt ]] && continue
    [[ "$p" == *.md ]] && continue
    compiler=1
    break
  done
  ((compiler == 0)) && exit 125
fi
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TDIR" \
  nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TDIR" \
  nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$TDIR"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$TOOLS/bisect_pin_test.py" "$WITNESS" /tmp/bisect_one_record.txt
