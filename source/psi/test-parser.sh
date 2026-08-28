#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)

if [ -n "${OMEGA_PRODUCT_PROGRAM:-}" ]; then
    python3 "$repo_root/source/psi/parse/test_parser.py" "$OMEGA_PRODUCT_PROGRAM"
    exit 0
fi

if [ -z "${OMEGA_CLI:-}" ] || [ -z "${OMEGA_TARGET:-}" ]; then
    echo "set OMEGA_CLI to the exact freshly built comparator CLI" >&2
    echo "and set OMEGA_TARGET to the exact selected target profile" >&2
    echo "or set OMEGA_PRODUCT_PROGRAM to an exact product executable" >&2
    exit 2
fi
omega_cli=$OMEGA_CLI
omega_target=$OMEGA_TARGET
if [ ! -x "$omega_cli" ]; then
    echo "missing Omega CLI: $omega_cli" >&2
    exit 1
fi

build_dir=$(mktemp -d "${TMPDIR:-/tmp}/omega-psi-parser.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM
"$omega_cli" --output-only --target "$omega_target" --build-dir "$build_dir" \
    "$repo_root/source/omega/main.omg"
python3 "$repo_root/source/psi/parse/test_parser.py" "$build_dir/omega-program"
