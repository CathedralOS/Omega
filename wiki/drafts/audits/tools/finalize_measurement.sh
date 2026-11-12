#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
AUDIT="$ROOT/wiki/drafts/audits"
python3 "$AUDIT/tools/merge_observed.py" \
  "$AUDIT/corpus/all_observed.txt" \
  "$AUDIT/corpus/batch_0.txt" \
  "$AUDIT/corpus/batch_1.txt" \
  "$AUDIT/corpus/batch_2.txt" \
  "$AUDIT/corpus/batch_3.txt" \
  "$AUDIT/corpus/batch_4.txt" \
  "$AUDIT/corpus/batch_5.txt" \
  "$AUDIT/corpus/batch_6.txt" \
  "$AUDIT/corpus/batch_7.txt" \
  "$AUDIT/corpus/host_settled.txt"
python3 - <<'PY'
import json
import subprocess
from pathlib import Path
import sys
sys.path.insert(0, "wiki/drafts/audits/tools")
from classify_corpus import classify_all
audit = Path("wiki/drafts/audits")
obs = audit / "corpus/all_observed.txt"
reg = audit / "regressed_fixtures.txt"
data = classify_all(obs, reg)
(audit / "measurement.json").write_text(
    json.dumps(
        {
            "commit": subprocess.check_output(["git", "rev-parse", "origin/main"], text=True).strip(),
            "host_desc": "linux x86_64",
            "counts": data["counts"],
            "host_only": data["host_only"],
        },
        indent=2,
    )
)
(audit / "classification.json").write_text(json.dumps(data, indent=2))
print("counts", data["counts"])
PY
python3 "$AUDIT/tools/cluster_broken.py" \
  "$AUDIT/corpus/all_observed.txt" \
  "$AUDIT/regressed_fixtures.txt" \
  "$AUDIT/clusters.json"
