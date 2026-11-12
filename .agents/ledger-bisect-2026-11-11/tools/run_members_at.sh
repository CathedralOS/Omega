#!/bin/bash
# Usage: run_members_at.sh <commit> member1 member2 ...
set -euo pipefail
COMMIT="${1:?}"; shift
MEMBERS=("$@")
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
cd "$ROOT"
git checkout -q "$COMMIT"
FULL=$(git rev-parse HEAD)
BISECT_TARGET="/tmp/omega-target-bisect-$(git rev-parse HEAD)"
mkdir -p "$BISECT_TARGET"
unset CARGO_TARGET_DIR
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx test -p compiler --test corpus_runner --no-run < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$BISECT_TARGET"/debug/build/compiler/*/out/corpus_runner-* | head -1)
FILTER=$(IFS=,; echo "${MEMBERS[*]}")
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library" OMEGA_CORPUS_JOBS=1
OUT="/tmp/members_at_${FULL}.txt"
export OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$OUT"
rm -f "$OUT"
nice -n 19 "$RUNNER" < /dev/null || true
echo "commit=$FULL"
python3 - "$OUT" "${MEMBERS[@]}" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, "/agent/repos/omega/.agents/ledger-bisect-2026-11-11/tools")
from classify_corpus import parse_records, is_worse_than_ledger

ledger_path = Path("/tmp/ledger-bisect-pin/corpus_outcomes.txt")
ledger = {}
for line in ledger_path.read_text().splitlines():
    if line.strip() and not line.startswith("\t") and " " in line:
        fix = line.split()[0]
        tier = fix.split("/")[0]
        st = line.split()[1]
        ledger[fix] = {"fixture": fix, "tier": tier, "status": st, "expected_satisfied": None, "diagnostics": []}

out = Path(sys.argv[1])
obs = parse_records(out.read_text()) if out.exists() else {}
for m in sys.argv[2:]:
    if m not in obs:
        print(f"{m}\tmissing\t\t")
        continue
    o = obs[m]
    o["tier"] = m.split("/")[0]
    p = ledger.get(m, {"status": "?", "expected_satisfied": None})
    bad = is_worse_than_ledger(p, o)
    d = (o.get("diagnostics") or [""])[0][:120]
    print(f"{m}\t{o['status']}\t{'broken' if bad else 'ok'}\t{d}")
PY
