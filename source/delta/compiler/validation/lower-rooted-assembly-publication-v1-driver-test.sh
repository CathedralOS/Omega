#!/usr/bin/env sh
# Focused bounded-stage tests for publication commands and evidence custody.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
python3 -B "$HERE/lower_rooted_assembly_publication_v1_driver_test.py"
echo "Delta lower-rooted assembly publication V1 driver tests PASS"
