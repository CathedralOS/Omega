#!/bin/bash
# Walk first-parent from GOOD toward PIN; find adjacent n where test goes 0 then 1.
set -uo pipefail
PIN="${BISECT_PIN:-51e8c45c793007f211d0d9baee7de16767172444}"
GOOD="${1:?good commit}"
WITNESS="${2:?witness}"
ROOT="$(git rev-parse --show-toplevel)"
TOOL="$ROOT/.agents/ledger-bisect-2026-11-11/tools/verify_at_commit.sh"
export BISECT_TOOLCACHE="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
MEASURE="$ROOT/.agents/ledger-bisect-2026-11-11/work/pin_measure.txt"
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/pin_bisect_context.py" "$WITNESS" "$MEASURE" >/dev/null

mapfile -t chain < <(git rev-list --first-parent "$PIN" | tac)
# chain[0]=oldest ... last=PIN
good_idx=-1
for i in "${!chain[@]}"; do
  [[ "$(git rev-parse "${chain[$i]}")" == "$(git rev-parse "$GOOD")" ]] && good_idx=$i && break
done
if ((good_idx < 0)); then
  echo "GOOD not on first-parent chain" >&2
  exit 1
fi

last_code=-1
last_good=""
first_bad=""
for ((i = good_idx; i < ${#chain[@]}; i++)); do
  sha="${chain[$i]}"
  if git diff --name-only "${sha}^1" "$sha" 2>/dev/null | grep -q .; then
    mapfile -t files < <(git diff --name-only "${sha}^1" "$sha" 2>/dev/null || true)
    ledger_only=1
    for f in "${files[@]}"; do
      [[ "$f" != tests/omega/corpus_outcomes.txt ]] && [[ ! "$f" =~ \.md$ ]] || continue
      ledger_only=0
      break
    done
    if ((ledger_only)); then
      echo "skip_ledger_only $sha" >&2
      continue
    fi
  fi
  set +e
  out=$("$TOOL" "$sha" "$WITNESS" 1 2>/dev/null)
  rc=$?
  set -e
  code=$(echo "$out" | python3 -c "import json,sys; print(json.loads(sys.stdin.read())['results'][0])" 2>/dev/null || echo 125)
  echo "n=$i $(git rev-parse --short "$sha") rc=$code" >&2
  if [[ "$code" == "125" ]]; then
    echo "skip_build $sha" >&2
    continue
  fi
  if [[ "$last_code" == "0" && "$code" == "1" ]]; then
  first_bad="$sha"
  last_good="${chain[$((i - 1))]}"
    echo "EDGE_GOOD=$last_good"
    echo "EDGE_BAD=$first_bad"
    echo "EDGE_PARENT=$(git rev-parse "${first_bad}^1")"
    exit 0
  fi
  last_code=$code
done
echo "NO_ISOLATED_EDGE last_code=$last_code" >&2
exit 2
