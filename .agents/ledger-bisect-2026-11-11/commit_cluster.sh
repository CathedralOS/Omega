#!/bin/bash
set -euo pipefail
CID="${1:?cluster id}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
python3 .agents/ledger-bisect-2026-11-11/render_part.py
git add .agents/ledger-bisect-2026-11-11/part-16.md .agents/ledger-bisect-2026-11-11/part-16.tsv .agents/ledger-bisect-2026-11-11/part16_meta.json ".agents/ledger-bisect-2026-11-11/bisect_${CID}.json" .agents/ledger-bisect-2026-11-11/tools
git commit -m "repo: ledger bisect part 16 cluster ${CID}"
git push -u origin work/ledger-bisect-part-16
