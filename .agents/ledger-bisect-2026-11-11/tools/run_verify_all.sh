#!/bin/bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
git checkout -qf work/ledger-bisect-part-26
for c in 0 1 2 3 4; do
  echo "=== verify cluster $c ==="
  python3 .agents/ledger-bisect-2026-11-11/tools/verify_cluster_edge.py "$c"
done
python3 .agents/ledger-bisect-2026-11-11/tools/write_part26_rows.py
git add .agents/ledger-bisect-2026-11-11/
git commit -m "repo: ledger bisect part 26 verified attribution" || true
git push origin work/ledger-bisect-part-26
