#!/bin/bash
# Pin ledger + witness PIN observation before git bisect (see lead methodology).
set -euo pipefail
WITNESS="${1:?witness fixture path}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
OBS="${3:-}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
git show "${PIN}:tests/omega/corpus_outcomes.txt" >/tmp/ledger_bisect_corpus_outcomes.txt
if [[ -z "$OBS" ]]; then
  OBS="$ROOT/.agents/ledger-bisect-2026-11-11/pin_observed.txt"
fi
python3 "$(dirname "$0")/prepare_witness_pin.py" "$WITNESS" /tmp/ledger_bisect_corpus_outcomes.txt "$OBS"
