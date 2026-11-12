#!/bin/bash
# Measure slice fixtures on PIN and bisect each resulting cluster witness.
set -euo pipefail
K="${1:-11}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
PIN="${LEDGER_BISECT_PIN:-51e8c45c793007f211d0d9baee7de16767172444}"
PART_DIR="$ROOT/.agents/ledger-bisect-2026-11-11"
TOOLS="$PART_DIR/tools"
python3 "$TOOLS/parse_report_slice.py" "$K" 40
FILTER="$(paste -sd, "$PART_DIR/slice${K}_fixtures.txt" | sed 's/,$//')"
OBS="$PART_DIR/pin_slice${K}_observed.txt"
"$TOOLS/run_filter_direct.sh" "$FILTER" "$OBS"
python3 - "$K" "$OBS" <<'PY'
import json, sys
from pathlib import Path
sys.path.insert(0, str(Path(".agents/ledger-bisect-2026-11-11/tools")))
from classify_corpus import classify_all, norm_diag
from collections import defaultdict

k, obs = sys.argv[1], Path(sys.argv[2])
slice_meta = json.loads(Path(f".agents/ledger-bisect-2026-11-11/slice{k}_from_report.json").read_text())
all_members = {m for c in slice_meta for m in c["members"]}
data = classify_all(obs, Path("/tmp/regressed_fixtures.txt"))
rows = []
for fix in sorted(all_members):
    if fix in data["fixed"]:
        rows.append((fix, "fixed_on_main", "", ""))
        continue
    detail = data["broken_detail"].get(fix, {})
    if fix not in data["broken_detail"]:
        if fix in data.get("missing", []):
            cls = "missing"
        else:
            cls = "fixed_on_main"
        rows.append((fix, cls, "", ""))
        continue
    rows.append((fix, "still_broken", detail.get("diag", "")[:500], norm_diag(detail.get("diag", ""))))
(Path(f".agents/ledger-bisect-2026-11-11/slice{k}_rows.tsv")).write_text(
    "fixture\tclass\tfirst_diagnostic\tcluster_id\n" + "\n".join("\t".join(r) for r in rows) + "\n"
)
broken = [r for r in rows if r[1] == "still_broken"]
clusters = defaultdict(list)
for fix, _, diag, norm in broken:
    clusters[norm or "(empty)"].append((fix, diag))
ranked = sorted(clusters.items(), key=lambda kv: (-len(kv[1]), kv[0]))
out = {"pin": "51e8c45c793007f211d0d9baee7de16767172444", "clusters": []}
for i, (norm, items) in enumerate(ranked):
    witness = max(items, key=lambda t: len(t[0]))[0]
    out["clusters"].append({
        "id": i,
        "rows": len(items),
        "diag_norm": norm,
        "diag_sample": items[0][1][:240],
        "witness": witness,
        "members": sorted(x[0] for x in items),
    })
Path(f".agents/ledger-bisect-2026-11-11/slice{k}_pin_clusters.json").write_text(json.dumps(out, indent=2))
print(f"fixed={sum(1 for r in rows if r[1]=='fixed_on_main')} broken={len(broken)} clusters={len(ranked)}")
PY

for CL in $(python3 -c "import json; print(' '.join(str(c['id']) for c in json.load(open('$PART_DIR/slice${K}_pin_clusters.json'))['clusters']))"); do
  WIT=$(python3 -c "import json; c=json.load(open('$PART_DIR/slice${K}_pin_clusters.json'))['clusters'][$CL]; print(c['witness'])")
  export BISECT_WITNESS="$WIT"
  export BISECT_GOOD=""
  BAD="$PIN" GOOD=$(python3 -c "
import sys
fix='$WIT'
for line in open('tests/omega/corpus_outcomes.txt'):
    if line.startswith(fix+' '):
        for tok in line.split():
            if tok.startswith('@'): print(tok[1:]); raise SystemExit
print('759c80404428')
")
  export GIT_PAGER=cat
  git bisect reset 2>/dev/null || true
  if ! git bisect start --first-parent "$BAD" "$GOOD" 2>/dev/null; then
    git bisect start
    git bisect bad "$BAD"
    git bisect good "$GOOD"
  fi
  set +e
  git bisect run "$TOOLS/bisect_witness.sh"
  RC=$?
  set -e
  BREAK=$(git rev-parse HEAD^{commit})
  PR=$(git log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
  MECH=$(git log -1 --format='%s' "$BREAK")
  git bisect reset
  python3 -c "
import json
from pathlib import Path
p=Path('$PART_DIR/bisect_slice${K}_cluster${CL}.json')
p.write_text(json.dumps({'witness':'$WIT','breaking_commit':'$BREAK','pr':'$PR','mechanism':'$MECH'[:300],'status':'bisected' if $RC in (0,1) else 'propagated','bisect_exit':$RC}, indent=2))
"
done
