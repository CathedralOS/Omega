#!/bin/bash
set -euo pipefail
SRC="$(cd "$(dirname "$0")/tools" && pwd)"
DST="/tmp/ledger_bisect_tools"
rm -rf "$DST"
mkdir -p "$DST"
cp "$SRC/bisect_test.py" "$SRC/bisect_witness.sh" "$DST/"
chmod +x "$DST/bisect_witness.sh"
export BISECT_TOOLS_ROOT="$DST"
