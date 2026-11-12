#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
FIX_DIR="${BISECT_FIXTURE_DIR:?set BISECT_FIXTURE_DIR}"
GOOD="${BISECT_GOOD:-$("$ROOT/.agents/ledger-bisect-2026-11-11/tools/find_good_commit.sh" "$WITNESS" "$PIN")}"
export BISECT_WITNESS="$WITNESS"
export BISECT_FIXTURE_DIR="$FIX_DIR"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export GIT_PAGER=cat
git checkout -q "$PIN"
git bisect reset 2>/dev/null || true
git bisect start --first-parent "$PIN" "$GOOD"
git bisect run "$SCRIPT"
git bisect log > "${FIX_DIR}/bisect.log" 2>&1 || true
BREAK=$(git rev-parse HEAD^{commit})
git bisect reset
git checkout -q "$PIN"
PR=$(git log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
echo "good=$GOOD break=$BREAK pr=${PR:-}"
git log -1 --oneline "$BREAK"
git show --stat "$BREAK" | head -30
