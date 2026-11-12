#!/bin/bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
export BISECT_PIN_LEDGER=/tmp/ledger-bisect-pin-corpus_outcomes.txt
export BISECT_PIN_META=/tmp/ledger-bisect-pin-meta.json
export BISECT_RECORD=/tmp/bisect_one_record.txt
exec bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
