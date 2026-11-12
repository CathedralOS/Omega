#!/bin/bash
# Copy bisect tools to /tmp so git checkout does not hide scripts.
set -euo pipefail
SRC="$(cd "$(dirname "$0")" && pwd)"
DEST=/tmp/ledger-bisect-tools
rm -rf "$DEST"
cp -a "$SRC" "$DEST"
export LEDGER_BISECT_TOOLS="$DEST"
echo "$DEST"
