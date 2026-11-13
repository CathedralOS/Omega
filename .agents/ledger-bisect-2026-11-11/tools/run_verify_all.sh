#!/bin/bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
git checkout -qf work/ledger-bisect-part-26
export LEDGER_BISECT_WORK=/tmp/ledger-bisect-work
rm -rf /tmp/.agents "$LEDGER_BISECT_WORK"
git archive HEAD .agents/ledger-bisect-2026-11-11/work | tar -x -C /tmp
cp -a /tmp/.agents/ledger-bisect-2026-11-11/work "$LEDGER_BISECT_WORK"
cp -a .agents/ledger-bisect-2026-11-11/tools/. /tmp/ledger-bisect-tools/
for c in 0 1 2 3 4; do
  echo "=== verify cluster $c $(date -u +%FT%TZ) ==="
  python3 .agents/ledger-bisect-2026-11-11/tools/verify_cluster_edge.py "$c"
done
mkdir -p .agents/ledger-bisect-2026-11-11/work
cp -a "$LEDGER_BISECT_WORK"/verify_c*.json .agents/ledger-bisect-2026-11-11/work/
export LEDGER_BISECT_WORK
python3 .agents/ledger-bisect-2026-11-11/tools/write_part26_rows.py
git add -f .agents/ledger-bisect-2026-11-11/
git commit -m "repo: ledger bisect part 26 verified attribution" || true
git push -u origin work/ledger-bisect-part-26
