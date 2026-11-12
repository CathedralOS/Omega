#!/bin/bash
set -uo pipefail
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT=/agent/repos/omega
WT=/tmp/bisect-wt
TOOL=$ROOT/.agents/ledger-bisect-2026-11-11/tools
STATE=$ROOT/.agents/ledger-bisect-2026-11-11/state.json

"$TOOL/prepare_pin_baseline.sh" "$PIN"
export BISECT_ROOT="$WT"
git -C "$WT" checkout -q "$PIN"

mapfile -t witnesses < <(python3 -c "
import json
s=json.load(open('$STATE'))
for c in sorted(s['pin_clusters'], key=lambda c: (-c['count'], c['witness'])):
    if c.get('bisect',{}).get('status') != 'bisected':
        print(c['witness'])
")

for w in "${witnesses[@]}"; do
  echo "=== bisect $w ===" | tee -a /tmp/ledger-bisect-all.log
  OUT="/tmp/bisect-$(echo "$w" | tr / _)"
  set +e
  nice -n 19 "$TOOL/run_bisect_cluster.sh" "$w" "$PIN" 2>&1 | tee "$OUT.log" | tee "$OUT.json" | python3 "$TOOL/apply_bisect_result.py"
  set -e
  python3 "$TOOL/write_part0.py" "$STATE"
  cd "$ROOT"
  git add .agents/ledger-bisect-2026-11-11/
  git commit -m "repo: ledger bisect part 0 cluster $(echo "$w" | tr / _ | cut -c1-60)"
  git push origin work/ledger-bisect-part-0
  git -C "$WT" checkout -q "$PIN"
done
