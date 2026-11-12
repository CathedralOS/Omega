#!/bin/bash
# Walk first-parent ancestors from PIN until witness returns 0 (fixed PIN ledger).
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
PIN="${PIN_COMMIT:?}"
ROOT="$(git rev-parse --show-toplevel)"
TOOLS="$(cd "$(dirname "$0")" && pwd)"
cp -a "$TOOLS" /tmp/ledger-bisect-tools
bash "$TOOLS/prepare_bisect_witness.sh" "$WITNESS" "$PIN"
cd "$ROOT"
while read -r rev; do
  git checkout "$rev" >/dev/null 2>&1 || continue
  export BISECT_WITNESS="$WITNESS"
  code=$(/tmp/ledger-bisect-tools/bisect_witness.sh; echo $?)
  echo "probe $rev exit=$code"
  if [[ "$code" == "0" ]]; then
    echo "GOOD=$rev"
    exit 0
  fi
done < <(git rev-list --first-parent "$PIN")
echo "GOOD=not_found" >&2
exit 1
