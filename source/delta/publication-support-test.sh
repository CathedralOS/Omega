#!/bin/sh
# Focused pre-publication custody gate. This does not publish or bless an artifact.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
command -v python3 >/dev/null 2>&1 || {
  echo "Delta publication support: python3 required" >&2
  exit 2
}

python3 -B "$HERE/publication_support_test.py"
echo "Delta publication support: exact LF image and strict Darwin-arm64 assembly controls passed"
