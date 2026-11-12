#!/bin/bash
# git bisect run with worktree steps between GOOD and BAD (first-parent).
set -euo pipefail
WITNESS="${1:?}"
GOOD="${2:?}"
BAD="${3:?}"
OUT="${4:?json path}"
ROOT="/agent/repos/omega"
LB="/tmp/ledger-bisect-2026-11-11"
rm -rf "$LB"
cp -a "$ROOT/.agents/ledger-bisect-2026-11-11" "$LB"
chmod +x "$LB/tools/"*.sh
export BISECT_WITNESS="$WITNESS"
STEP="$LB/tools/bisect_step_worktree.sh"
chmod +x "$STEP" "$LB/tools/find_good_exponential.sh" 2>/dev/null || true
cd "$ROOT"
git checkout -q work/ledger-bisect-part-5
git bisect reset "$BAD" 2>/dev/null || true
set +e
git bisect start --first-parent "$BAD" "$GOOD"
BLOG="${BISECT_LOG:-${OUT%.json}.log}"
git bisect run "$STEP" 2>&1 | tee "$BLOG"
BS=$?
set -e
FIRST=$(git rev-parse HEAD)
fb=$(grep -E 'first bad commit|is the first bad commit' "$BLOG" 2>/dev/null | tail -1 || true)
if [[ -n "$fb" ]]; then
  sha=$(sed -n 's/.*\[\([0-9a-f]\{7,\}\).*/\1/p' <<<"$fb" | head -1)
  [[ -n "$sha" ]] && FIRST=$(git rev-parse "$sha")
fi
SUBJ=$(git log -1 --format='%s' "$FIRST")
PR=$(sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' <<<"$SUBJ" | head -1)
PARENT=$(git rev-parse "${FIRST}^1" 2>/dev/null || echo "")
git bisect reset "$BAD" 2>/dev/null || true
git checkout -q work/ledger-bisect-part-5
python3 - "$OUT" "$WITNESS" "$GOOD" "$BAD" "$FIRST" "$PARENT" "$PR" "$SUBJ" "$BS" <<'PY'
import json, subprocess, sys
out, witness, good, bad, first, parent, pr, subj, bs = sys.argv[1:10]
mech = ""
if first:
    r = subprocess.run(
        ["git", "-C", "/agent/repos/omega", "show", "--stat", "--format=", first],
        capture_output=True,
        text=True,
    )
    mech = "\n".join(
        ln for ln in r.stdout.splitlines()
        if not ln.startswith(" tests/omega/corpus_outcomes") and not ln.endswith(".md")
    )[:500]
payload = {
    "witness": witness,
    "good": good,
    "bad": bad,
    "breaking_commit": first,
    "parent": parent,
    "pr": pr or None,
    "mechanism": subj[:280],
    "mechanism_detail": mech,
    "bisect_exit": int(bs),
    "status": "bisected" if int(bs) == 0 and first != bad else "incomplete",
}
open(out, "w").write(json.dumps(payload, indent=2))
print(first, pr, subj[:72])
PY
