#!/bin/bash
# Steps 2–4 for one cluster: 3x parent, 3x bad, member matrix, mechanism hunk quote.
set -euo pipefail
CLUSTER="${1:?index 0-4}"
ROOT="$(git rev-parse --show-toplevel)"
DIR="$ROOT/.agents/ledger-bisect-2026-11-11"
TOOL="$DIR/tools/verify_at_commit.sh"
BJSON="$DIR/work/bisect_c${CLUSTER}.json"
OUT="$DIR/work/verify_c${CLUSTER}.json"
[[ -f "$BJSON" ]] || { echo "missing $BJSON" >&2; exit 1; }

read -r WITNESS PARENT BAD GOOD ANCHOR <<<"$(python3 - <<PY
import json
d=json.load(open("$BJSON"))
bad=d["breaking_commit"]
parent=subprocess_check(bad)
good=d.get("good","")
anchor=d.get("anchor","741270df60f7")
witness=d["witness"]
import subprocess
def subprocess_check(bad):
    import subprocess
    return subprocess.check_output(["git","rev-parse",f"{bad}^1"],text=True).strip()
print(witness, parent, bad, good, anchor)
PY
)"

read -r -a MEMBERS <<<"$(python3 -c "import json; c=json.load(open('$DIR/work/pin_clusters.json'))[$CLUSTER]; print(' '.join(c['members']))")"

python3 "$DIR/tools/pin_bisect_context.py" "$WITNESS" "$DIR/work/pin_measure.txt" >/dev/null

parent_trials=$("$TOOL" "$PARENT" "$WITNESS" 3)
bad_trials=$("$TOOL" "$BAD" "$WITNESS" 3)

members=()
for m in "${MEMBERS[@]}"; do
  python3 "$DIR/tools/pin_bisect_context.py" "$m" "$DIR/work/pin_measure.txt" >/dev/null
  p=$("$TOOL" "$PARENT" "$m" 1)
  b=$("$TOOL" "$BAD" "$m" 1)
  members+=("$(python3 -c "import json; print(json.dumps({'fixture': '$m', 'parent': json.loads('''$p'''), 'bad': json.loads('''$b''')}))")")
done

HUNK=$(git show "$BAD" --format= --unified=3 -- 'omega-rust/psi/**' ':(exclude)omega-rust/psi/**/lowered-discharge/**' 2>/dev/null | head -c 12000)
if [[ -z "$HUNK" ]]; then
  HUNK=$(git show "$BAD" --format= --unified=3 -- 'omega-rust/psi/**' 2>/dev/null | head -c 12000)
fi
[[ -n "$HUNK" ]] || HUNK="mechanism not established"

PR=$(git log -1 --format='%s' "$BAD" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)

python3 - "$OUT" <<PY
import json, sys
out = {
  "cluster": int("$CLUSTER"),
  "witness": "$WITNESS",
  "parent": "$PARENT",
  "first_bad": "$BAD",
  "good_anchor": "$(git rev-parse "$GOOD" 2>/dev/null || echo "$GOOD")",
  "pin": "51e8c45c793007f211d0d9baee7de16767172444",
  "pr": "$PR" or None,
  "parent_trials": json.loads('''$parent_trials'''),
  "bad_trials": json.loads('''$bad_trials'''),
  "members": [$(
    printf '%s,' "${members[@]}" | sed 's/,$//'
  )],
  "mechanism_hunk_excerpt": """$HUNK""",
}
open("$OUT", "w").write(json.dumps(out, indent=2) + "\n")
print("wrote", "$OUT")
PY
