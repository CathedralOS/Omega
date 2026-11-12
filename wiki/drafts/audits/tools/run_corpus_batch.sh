#!/bin/bash
# Run one corpus batch (≤200 filters) via direct corpus_runner (no xtask 1800s cap).
set -euo pipefail
BATCH_ID="${1:?batch id}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
exec "$ROOT/wiki/drafts/audits/tools/run_corpus_batch_direct.sh" "$BATCH_ID"
