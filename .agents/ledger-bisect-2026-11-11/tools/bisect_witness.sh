#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
if git rev-parse --verify HEAD^1 >/dev/null 2>&1; then
  mapfile -t touched < <(git diff --name-only HEAD^1 HEAD 2>/dev/null || true)
  if ((${#touched[@]} > 0)); then
    only_ledger=1
    for p in "${touched[@]}"; do
      if [[ "$p" == tests/omega/corpus_outcomes.txt ]]; then continue; fi
      case "$p" in *.md) ;; *) only_ledger=0; break ;; esac
    done
    if ((only_ledger)); then exit 125; fi
  fi
fi
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS"
export OMEGA_CORPUS_RECORDS="${BISECT_RECORD:-/tmp/bisect_one_record.txt}"
rm -f "${BISECT_RECORD:-/tmp/bisect_one_record.txt}"
bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_run_fixture.sh" || exit 125
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_pin_test.py"
