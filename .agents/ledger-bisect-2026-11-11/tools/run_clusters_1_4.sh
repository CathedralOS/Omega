#!/bin/bash
set -euo pipefail
cd /agent/repos/omega
export MBX_SCHEDULER_CPUS=8 CARGO_BUILD_JOBS=8 OMEGA_CORPUS_JOBS=1
git checkout work/ledger-bisect-part-26
WITNESSES=(
  "pass/calls/runtime_constructed_borrowed_payload_arguments_exit"
  "pass/borrows/copied_intermediate_descriptor_snapshot_exit"
  "pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit"
  "pass/borrows/runtime_indexed_bounded_primitive_loan_write_exit"
)
for i in 1 2 3 4; do
  w="${WITNESSES[$((i-1))]}"
  echo "=== cluster $i witness $w ==="
  .agents/ledger-bisect-2026-11-11/tools/run_bisect_cluster.sh "$w" ".agents/ledger-bisect-2026-11-11/work/bisect_c${i}.json" \
    | tee ".agents/ledger-bisect-2026-11-11/work/bisect_c${i}.run.log"
  # recover breaking from bisect log if needed
  if grep -q 'possible first bad commit:' ".agents/ledger-bisect-2026-11-11/work/bisect_c${i}.json.log" 2>/dev/null; then
    python3 - "$i" <<'PY'
import json, re, subprocess, sys
from pathlib import Path
i = int(sys.argv[1])
out = Path(f".agents/ledger-bisect-2026-11-11/work/bisect_c{i}.json")
log = Path(f".agents/ledger-bisect-2026-11-11/work/bisect_c{i}.json.log")
data = json.loads(out.read_text())
if data.get("breaking_commit"):
    raise SystemExit(0)
cands = re.findall(r"\[([0-9a-f]{40})\]", "\n".join(
    l for l in log.read_text().splitlines() if "possible first bad" in l
))
for c in cands:
    diff = subprocess.check_output(["git", "diff", "--name-only", f"{c}^1", c], text=True)
    if diff.strip() and all(
        ln == "tests/omega/corpus_outcomes.txt" or ln.endswith(".md") for ln in diff.splitlines()
    ):
        continue
    subj = subprocess.check_output(["git", "log", "-1", "--format=%s", c], text=True).strip()
    pr = re.search(r"#(\d+)", subj)
    data.update(
        breaking_commit=c,
        pr=int(pr.group(1)) if pr else None,
        mechanism=subj,
        status="bisected",
        bisect_exit=0,
    )
    out.write_text(json.dumps(data, indent=2) + "\n")
    print("patched", c[:12], subj)
    break
PY
  fi
  python3 .agents/ledger-bisect-2026-11-11/tools/write_part26.py
  git add .agents/ledger-bisect-2026-11-11/part-26.md .agents/ledger-bisect-2026-11-11/part-26.tsv \
    .agents/ledger-bisect-2026-11-11/work/bisect_c${i}.json .agents/ledger-bisect-2026-11-11/work/bisect_c${i}.pin_context.json \
    .agents/ledger-bisect-2026-11-11/work/bisect_c${i}.json.log .agents/ledger-bisect-2026-11-11/work/bisect_c${i}.run.log 2>/dev/null || true
  git commit -m "repo: ledger bisect part 26 cluster ${i}" || true
  git push origin work/ledger-bisect-part-26
done
