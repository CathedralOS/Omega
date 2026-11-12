#!/bin/bash
set -euo pipefail
ROOT="/agent/repos/omega"
PIN=51e8c45c793007f211d0d9baee7de16767172444
BRANCH=work/ledger-bisect-part-14
TOOLS="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
cd "$ROOT"
git checkout -f "$BRANCH"
FILTER=$(paste -sd, /tmp/part14_fixtures.txt)
export CARGO_TARGET_DIR=/tmp/omega-bisect-target-measure
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$CARGO_TARGET_DIR nice -n 19 mbx build -p omega < /dev/null
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$CARGO_TARGET_DIR nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null
RUNNER=$(ls -t "$CARGO_TARGET_DIR"/debug/build/compiler/*/out/corpus_runner-* | head -1)
export OMEGA_CORPUS_ROOT=$ROOT OMEGA_LIBRARY_ROOT=$ROOT/source/library OMEGA_CORPUS_JOBS=1
export OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS=/tmp/part14_records.txt
nice -n 19 "$RUNNER" < /dev/null
bash "$TOOLS/setup_pin_fixtures.sh" /tmp/part14_records.txt
python3 << 'PY'
import json, sys
from collections import defaultdict
from pathlib import Path
sys.path.insert(0, str(Path(".agents/ledger-bisect-2026-11-11/tools")))
from classify_corpus import parse_records, is_worse_than_ledger, norm_diag
golden = parse_records(Path("tests/omega/corpus_outcomes.txt").read_text())
observed = parse_records(Path("/tmp/part14_records.txt").read_text())
regressed = Path("/tmp/part14_fixtures.txt").read_text().splitlines()
clusters = defaultdict(list)
fixed = []
for fix in regressed:
    obs = observed[fix]; prior = golden[fix]
    d = (obs["diagnostics"] or [""])[0]
    worse = is_worse_than_ledger(prior, obs)
    if worse:
        clusters[norm_diag(d)].append({"fixture": fix, "class": "still_broken", "diag": d, "kind": "pass/run rejected" if fix.startswith("pass/") else "fail wrong reason"})
    else:
        fixed.append({"fixture": fix, "class": "fixed_on_main", "diag": ""})
witnesses = Path("/tmp/part14_cluster_witnesses.txt").read_text().splitlines()
ranked = []
for w in witnesses:
    for members in clusters.values():
        if any(m["fixture"] == w for m in members):
            ranked.append({"witness": w, "diag_sample": members[0]["diag"], "kind": members[0]["kind"], "members": sorted(members, key=lambda x: x["fixture"])})
json.dump({"clusters": ranked, "fixed": fixed}, open("/tmp/part14_clusters_meta.json","w"), indent=2)
Path("/tmp/part14_broken.txt").write_text("\n".join(m["fixture"] for c in ranked for m in c["members"]))
Path("/tmp/part14_fixed.txt").write_text("\n".join(f["fixture"] for f in fixed))
PY
declare -A PRE_GOOD
PRE_GOOD[pass/build/component_declaration_scalar_conditions_exit]=145f7de4431272375ad294ce507901379921a6d1
cluster=0
mapfile -t WITNESSES < /tmp/part14_cluster_witnesses.txt
for w in "${WITNESSES[@]}"; do
  echo "=== CLUSTER $cluster WITNESS $w ==="
  OUT="/tmp/bisect_c${cluster}.json"
  export BISECT_GOOD="${PRE_GOOD[$w]:-}"
  bash "$TOOLS/run_pin_bisect.sh" "$w" "$PIN" "$BRANCH" "$OUT" || true
  python3 "$ROOT/.agents/ledger-bisect-2026-11-11/append_cluster.py" "$cluster" "$OUT"
  git add .agents/ledger-bisect-2026-11-11/
  git commit -m "repo: ledger bisect part 14 cluster ${cluster}"
  git push -u origin "$BRANCH"
  unset BISECT_GOOD
  cluster=$((cluster + 1))
done
python3 << 'PY'
from pathlib import Path
md = Path(".agents/ledger-bisect-2026-11-11/part-14.md")
append = "\n## Fixed on main\n" + "".join(f"- `{l}`\n" for l in Path("/tmp/part14_fixed.txt").read_text().splitlines())
append += "\n## Commands\n"
append += "- PIN `51e8c45c793` measured with direct corpus_runner, jobs=1\n"
append += f"- `.agents/ledger-bisect-2026-11-11/tools/bisect_pin_witness.sh` + `git bisect run --first-parent`\n"
md.write_text(md.read_text() + append)
PY
