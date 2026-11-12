#!/bin/bash
# Exit 0 if witness matches ledger expectation at current HEAD, 1 if regression, 125 build/skip.
set -euo pipefail
WITNESS="${1:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
"$SCRIPT"
