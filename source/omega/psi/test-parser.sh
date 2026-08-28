#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)

if [ -n "${OMEGA_PRODUCT_PROGRAM:-}" ]; then
    python3 "$repo_root/source/omega/psi/parse/test_parser.py" "$OMEGA_PRODUCT_PROGRAM"
    exit 0
fi

omega_cli=${OMEGA_CLI:-"$repo_root/target/debug/omega"}
if [ ! -x "$omega_cli" ]; then
    echo "missing Omega CLI: $omega_cli" >&2
    echo "build it once with: cargo build --locked --offline -p omega" >&2
    exit 1
fi

build_dir=$(mktemp -d "${TMPDIR:-/tmp}/omega-psi-parser.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM
"$omega_cli" --output-only --build-dir "$build_dir" "$repo_root/source/omega/main.omg"
python3 "$repo_root/source/omega/psi/parse/test_parser.py" "$build_dir/omega-program"
