#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT="${2:?output json}"
ROOT="/agent/repos/omega"
PIN="51e8c45c793007f211d0d9baee7de16767172444"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
cd "$ROOT"
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/prepare_bisect_pin.py" "$WITNESS"
GOOD="${BISECT_GOOD:-$("$ROOT/.agents/ledger-bisect-2026-11-11/tools/find_good_commit.sh" "$WITNESS")}"
export BISECT_WITNESS="$WITNESS"
: >"$OUT.endpoints"
for c in "$GOOD" 741270df60f7eab218e879eb6771d2bc75e64c00 "$PIN"; do
  git checkout -q "$c"
  set +e; "$SCRIPT"; rc=$?; set -e
  echo "endpoint $c rc=$rc" >>"$OUT.endpoints"
done
git checkout -q work/ledger-bisect-part-5
git bisect reset "$PIN" 2>/dev/null || true
export BISECT_WITNESS="$WITNESS"
set +e
git bisect start --first-parent "$PIN" "$GOOD"
git bisect run env BISECT_WITNESS="$WITNESS" "$SCRIPT" 2>&1 | tee "$OUT.log"
BS=$?
set -e
FIRST=$(git rev-parse HEAD)
if [[ "$BS" -ne 0 || "$FIRST" == "$PIN" ]]; then
  fb=$(grep -E 'first bad commit|is the first bad commit' "$OUT.log" | tail -1 || true)
  if [[ -n "$fb" ]]; then
    FIRST=$(sed -n 's/.*\[\([0-9a-f]\{7,\}\).*/\1/p' <<<"$fb" | head -1)
    [[ -n "$FIRST" ]] && FIRST=$(git rev-parse "$FIRST")
  fi
fi
SUBJ=$(git log -1 --format='%s' "$FIRST")
PR=$(sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' <<<"$SUBJ" | head -1)
MECH=$(git show --stat --format= "$FIRST" | grep -vE '^ tests/omega/corpus_outcomes|^ .*\.md$' | head -8 | tr '\n' ' ')
if [[ "$BS" -ne 0 || "$FIRST" == "$PIN" ]]; then
  STATUS=propagated
elif printf '%s' "$(git diff --name-only "${FIRST}^1" "$FIRST" 2>/dev/null)" | grep -qvE '^(tests/omega/corpus_outcomes\.txt|.*\.md)$'; then
  STATUS=bisected
else
  STATUS=propagated
fi
git bisect reset "$PIN" 2>/dev/null || true
git checkout -q work/ledger-bisect-part-5
python3 - "$OUT" "$WITNESS" "$GOOD" "$PIN" "$FIRST" "$PR" "$SUBJ" "$MECH" "$BS" "$STATUS" <<'PY'
import json, sys
out, witness, good, pin, first, pr, subj, mech, bs, status = sys.argv[1:11]
open(out, "w").write(
    json.dumps(
        {
            "witness": witness,
            "good": good,
            "bad": pin,
            "breaking_commit": first,
            "pr": pr or None,
            "mechanism": subj[:280],
            "mechanism_detail": mech[:500],
            "bisect_exit": int(bs),
            "status": status,
        },
        indent=2,
    )
)
print(first, status, pr, subj[:80])
PY
