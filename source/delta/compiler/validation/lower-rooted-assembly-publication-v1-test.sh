#!/usr/bin/env sh
# Focused platform-neutral tests for the exact assembly publication join.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
python3 -B "$HERE/lower_rooted_assembly_publication_v1_test.py"
echo "Delta lower-rooted assembly publication V1 tests PASS"
