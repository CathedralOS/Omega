#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)

if [ -z "${OMEGA_CLI:-}" ] || [ -z "${OMEGA_TARGET:-}" ] \
    || [ -z "${OMEGA_LEXER_OBSERVER:-}" ]; then
    echo "set OMEGA_CLI to the exact freshly built comparator CLI" >&2
    echo "set OMEGA_TARGET to the exact selected target profile" >&2
    echo "and set OMEGA_LEXER_OBSERVER to the exact Rust lexer observer" >&2
    exit 2
fi
omega_cli=$OMEGA_CLI
target=$OMEGA_TARGET
omega_lexer_observer=$OMEGA_LEXER_OBSERVER
if [ ! -x "$omega_cli" ]; then
    echo "missing Omega CLI: $omega_cli" >&2
    exit 1
fi
if [ ! -x "$omega_lexer_observer" ]; then
    echo "missing Rust lexer observer: $omega_lexer_observer" >&2
    exit 1
fi

build_dir=$(mktemp -d "${TMPDIR:-/tmp}/omega-psi-parser.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM
"$omega_cli" --output-only --target "$target" --build-dir "$build_dir" \
    "$repo_root/source/psi/gates/parser/main.omg"
cli_identity=$(shasum -a 256 "$omega_cli" | awk '{print $1}')
artifact_identity=$(shasum -a 256 "$build_dir/omega-program" | awk '{print $1}')
observer_identity=$(shasum -a 256 "$omega_lexer_observer" | awk '{print $1}')
printf 'parser-gate cli_sha256=%s target=%s artifact_sha256=%s lexer_observer_sha256=%s\n' \
    "$cli_identity" "$target" "$artifact_identity" "$observer_identity"
python3 "$repo_root/source/psi/parse/test_parser.py" \
    "$build_dir/omega-program" "$omega_lexer_observer"
