#!/bin/sh
# Shared driver for the product compiler gates. A gate names the package it
# builds and a semantic-free Python decoder: the Psi gates under tests/psi/
# build their own harness packages around source/psi, and the product gates
# under tests/product/ build their actual production application roots.
# `gate.sh <name> <package-directory> <decoder.py>` builds the package's
# program natively once, unlocked, and runs the decoder's cases against that
# artifact.
set -eu

if [ "$#" -lt 3 ]; then
    echo "usage: gate.sh <name> <package-directory> <decoder.py> [decoder-argument...]" >&2
    exit 2
fi
gate_name=$1
gate=$(CDPATH= cd -- "$2" && pwd)
decoder=$(CDPATH= cd -- "$(dirname -- "$3")" && pwd)/$(basename -- "$3")
shift 3

if [ -z "${OMEGA_CLI:-}" ] || [ -z "${OMEGA_TARGET:-}" ]; then
    echo "set OMEGA_CLI to the exact freshly built comparator CLI" >&2
    echo "set OMEGA_TARGET to the exact selected target profile" >&2
    exit 2
fi
omega_cli=$OMEGA_CLI
target=$OMEGA_TARGET
if [ ! -x "$omega_cli" ]; then
    echo "missing Omega CLI: $omega_cli" >&2
    exit 1
fi

# The gate compiles its package once, with no lock. An unlocked project
# compiles natively through the same package route and every language,
# proof, representation and native check
# (wiki/spec/packages/acceptance.md#ordinary-compilation); what it lacks is
# accepted state: semantic bindings, restricted-build grants and receiver
# permissions, none of which a gate Build asks for. Reviewing first with
# `omega packages update` and accepting every decision unread compiled the
# whole package graph a second time, and validated every unused std and psi
# declaration, without auditing anything. A lock or review state left by
# another checkout or compiler is removed so it can neither block the build
# nor stand in for a review.
rm -f "${gate:?}/omega.lock" "${gate:?}/omega.admissions"
rm -rf "${gate:?}/build/package-manager"

gate_outputs="$gate/build/product-gates"
mkdir -p "$gate_outputs"
build_dir=$(mktemp -d "$gate_outputs/$gate_name.XXXXXX")
cleanup_gate() {
    gate_status=$?
    trap - EXIT HUP INT TERM
    if [ "$gate_status" -eq 0 ]; then
        rm -rf "$build_dir"
    else
        printf '%s-gate failed; build directory retained: %s\n' "$gate_name" "$build_dir" >&2
    fi
    exit "$gate_status"
}
trap cleanup_gate EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
"$omega_cli" --timings --target "$target" --build-dir "$build_dir" "$gate/main.omg"
cli_identity=$(shasum -a 256 "$omega_cli" | awk '{print $1}')
artifact_identity=$(shasum -a 256 "$build_dir/omega-program" | awk '{print $1}')
printf '%s-gate cli_sha256=%s target=%s artifact_sha256=%s\n' \
    "$gate_name" "$cli_identity" "$target" "$artifact_identity"
python3 "$decoder" "$build_dir/omega-program" "$@"
