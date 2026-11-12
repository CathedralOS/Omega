#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
BRANCH="${3:-work/ledger-bisect-part-14}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
KNOWN_BAD="${4:-741270df60f7eab218e879eb6771d2bc75e64c00}"
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/find_pin_good.py" "$WITNESS" "$PIN" "$BRANCH" "$KNOWN_BAD"
