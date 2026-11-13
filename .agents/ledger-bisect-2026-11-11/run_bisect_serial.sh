#!/bin/bash
# Resume part-4 bisects. Does not reset the branch and does not push main.
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git fetch origin work/ledger-bisect-part-4
git checkout work/ledger-bisect-part-4
git pull --ff-only origin work/ledger-bisect-part-4
if [[ ! -f "$ROOT/.agents/ledger-bisect-wt/.git" ]]; then
  git worktree add -f "$ROOT/.agents/ledger-bisect-wt" 51e8c45c793007f211d0d9baee7de16767172444
fi
exec python3 "$ROOT/.agents/ledger-bisect-2026-11-11/run_cluster.py" --all
