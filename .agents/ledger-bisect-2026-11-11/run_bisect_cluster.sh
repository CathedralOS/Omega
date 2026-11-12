#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
CLUSTER_ID="${2:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git show "51e8c45c793007f211d0d9baee7de16767172444:tests/omega/corpus_outcomes.txt" \
  >/tmp/ledger-bisect-pin-corpus_outcomes.txt
export BISECT_REPO_ROOT="$ROOT"
nice -n 19 python3 "$ROOT/.agents/ledger-bisect-2026-11-11/run_cluster.py" "$WITNESS" "$CLUSTER_ID"
