#!/bin/bash
# Run a single cluster bisect + part-26 commit (cluster index 0-4).
set -euo pipefail
idx="${1:?cluster index}"
cd "$(git rev-parse --show-toplevel)"
git checkout -qf work/ledger-bisect-part-26
WITNESSES=(
  "pass/termination/nested_helper_record_measure_result_equation_exit"
  "pass/calls/runtime_constructed_borrowed_payload_arguments_exit"
  "pass/borrows/copied_intermediate_descriptor_snapshot_exit"
  "pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit"
  "pass/borrows/runtime_indexed_bounded_primitive_loan_write_exit"
)
w="${WITNESSES[$idx]}"
out=".agents/ledger-bisect-2026-11-11/work/bisect_c${idx}.json"
echo "=== cluster $idx witness $w ==="
.agents/ledger-bisect-2026-11-11/tools/run_bisect_cluster.sh "$w" "$out" \
  | tee ".agents/ledger-bisect-2026-11-11/work/bisect_c${idx}.run.log"
if [[ -f "${out}.log" ]] && ! jq -e '.breaking_commit' "$out" >/dev/null 2>&1; then
  python3 - "$idx" <<'PY'
import json, re, subprocess, sys
from pathlib import Path
i = int(sys.argv[1])
out = Path(f".agents/ledger-bisect-2026-11-11/work/bisect_c{i}.json")
log = Path(f".agents/ledger-bisect-2026-11-11/work/bisect_c{i}.json.log")
if not out.is_file():
    raise SystemExit(0)
data = json.loads(out.read_text())
if data.get("breaking_commit"):
    raise SystemExit(0)
if not log.is_file():
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
  ".agents/ledger-bisect-2026-11-11/work/bisect_c${idx}.json" ".agents/ledger-bisect-2026-11-11/work/bisect_c${idx}.pin_context.json" \
  ".agents/ledger-bisect-2026-11-11/work/bisect_c${idx}.json.log" ".agents/ledger-bisect-2026-11-11/work/bisect_c${idx}.run.log" 2>/dev/null || true
git commit -m "repo: ledger bisect part 26 cluster ${idx}" || true
git push origin work/ledger-bisect-part-26
