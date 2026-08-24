#!/bin/sh
set -eu

checkpoint_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
checkpoint_root=$(CDPATH= cd -- "$checkpoint_dir/../.." && pwd)

cd "$checkpoint_root"
python3 compiler/source-checkpoints/verify_manifest.py
