#!/bin/bash
# Retired. The old witness treated a different rejection as skip (exit 125),
# and this script then linear-scanned. Part 4 now runs run_cluster.py.
echo "bisect_one.sh is retired; run .agents/ledger-bisect-2026-11-11/run_cluster.py" >&2
exit 2
