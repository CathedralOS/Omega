#!/bin/bash
set -euo pipefail
PIN="${BISECT_PIN:?}"
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
TOOLS="$(dirname "$0")"
STATE="/tmp/ledger-bisect-part-33"
mkdir -p "$STATE"
slug="${WITNESS//\//_}"

export BISECT_PIN_LEDGER="$STATE/pin_corpus_outcomes.txt"
export BISECT_PIN_OBSERVATION="$STATE/pin_observation.txt"
export BISECT_WITNESS_SPEC="$STATE/${slug}.json"
export BISECT_REPO_ROOT="$ROOT"
export BISECT_WITNESS="$WITNESS"
export BISECT_WITNESS_SCRIPT="/tmp/ledger-bisect-witness-run.sh"

cp "$ROOT/tests/omega/corpus_outcomes.txt" "$BISECT_PIN_LEDGER"
[[ -f "$BISECT_PIN_OBSERVATION" ]] || { echo "missing $BISECT_PIN_OBSERVATION" >&2; exit 1; }

python3 "$TOOLS/prepare_witness.py" "$WITNESS"
cat >"$BISECT_WITNESS_SCRIPT" <<EOF
#!/bin/bash
export BISECT_WITNESS='$WITNESS'
export BISECT_REPO_ROOT='$ROOT'
export BISECT_WITNESS_SPEC='$BISECT_WITNESS_SPEC'
exec bash '$TOOLS/bisect_witness.sh'
EOF
chmod +x "$BISECT_WITNESS_SCRIPT"

export BISECT_PIN="$PIN"
good="$(bash "$TOOLS/find_good_commit.sh")"
echo "GOOD=$good" | tee "$STATE/${slug}.good"
cd "$ROOT"
git checkout -q work/ledger-bisect-part-33 2>/dev/null || git checkout -q "$PIN"
git bisect reset >/dev/null 2>&1 || true
export BISECT_WITNESS BISECT_REPO_ROOT BISECT_WITNESS_SPEC
git bisect start --first-parent "$PIN" "$good"
git bisect run "$BISECT_WITNESS_SCRIPT" 2>&1 | tee "$STATE/${slug}.bisect.log"
git bisect log >"$STATE/${slug}.bisect_log.txt"
first_bad="$(grep -m1 '^# first bad commit:' "$STATE/${slug}.bisect_log.txt" | sed 's/^# first bad commit: //' | cut -d']' -f1 | tr -d '[]' | awk '{print $1}')"
echo "FIRST_BAD=$first_bad"
