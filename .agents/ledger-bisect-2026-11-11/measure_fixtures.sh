#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
FILTER="${1:?comma-separated fixtures}"
OUT="${2:?output records file}"
export OMEGA_CORPUS_JOBS=1
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
if [[ ! -x "$RUNNER" ]]; then
  echo "missing corpus_runner; build omega first" >&2
  exit 1
fi
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$OUT"
LEDGER_PIN="${LEDGER_BISECT_PIN_LEDGER:-/tmp/ledger_bisect_part23_corpus_outcomes.txt}"
# If every filtered fixture shares one check-target flag, enable native check.
if [[ -f "$LEDGER_PIN" ]]; then
  TARGETS=$(python3 - "$FILTER" "$LEDGER_PIN" << 'PY'
import sys
fixes = [f.strip() for f in sys.argv[1].split(',') if f.strip()]
text = open(sys.argv[2]).read().splitlines()
flags = set()
for fix in fixes:
    for line in text:
        if line.startswith(fix + " "):
            for tok in line.split():
                if tok.startswith("check-target:"):
                    flags.add(tok.split(":", 1)[1])
            break
if len(flags) == 1:
    print(flags.pop())
PY
)
  if [[ -n "$TARGETS" ]]; then
    export OMEGA_CORPUS_NATIVE=1 OMEGA_CORPUS_NATIVE_TARGET="$TARGETS"
  fi
fi
rm -f "$OUT"
nice -n 19 "$RUNNER" < /dev/null
wc -l "$OUT"
