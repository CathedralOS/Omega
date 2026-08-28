#!/usr/bin/env sh
# Focused no-long-run tests for publication prepare/status/finalize custody.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
python3 -B "$HERE/lower_rooted_assembly_publication_v1_driver_test.py"
echo "Delta lower-rooted assembly publication V1 driver tests PASS"
