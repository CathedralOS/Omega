#!/bin/sh
# Shared driver for the product compiler gates. A gate names the package it
# builds and a semantic-free Python decoder: the Psi gates under tests/psi/
# build their own harness packages around source/psi, and the product gates
# under tests/product/ build the product root source/omega itself.
# `gate.sh <name> <package-directory> <decoder.py>` reviews the package,
# builds its program natively once and runs the decoder's cases against that
# artifact.
set -eu

if [ "$#" -ne 3 ]; then
    echo "usage: gate.sh <name> <package-directory> <decoder.py>" >&2
    exit 2
fi
gate_name=$1
gate=$(CDPATH= cd -- "$2" && pwd)
decoder=$(CDPATH= cd -- "$(dirname -- "$3")" && pwd)/$(basename -- "$3")

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

# The program entry needs this checkout's accepted lock. The lock and review
# state are host-local and ignored; accepting every pending decision records
# project acceptance for the measurement, not that an audit occurred
# (tools/sample_native_gate.py follows the same rule). Each run starts from
# no accepted state, so a lock or review left by another checkout or
# compiler can neither block the run nor stand in for its review.
# Exit 3 reports pending review decisions, which the next step accepts.
rm -f "${gate:?}/omega.lock" "${gate:?}/omega.admissions"
rm -rf "${gate:?}/build/package-manager"
review_status=0
"$omega_cli" update --project "$gate" --target "$target" || review_status=$?
if [ "$review_status" -ne 0 ] && [ "$review_status" -ne 3 ]; then
    exit "$review_status"
fi
review="$gate/build/package-manager/review-$target.txt"
if [ -f "$review" ] && grep -Eq '^decision row [0-9a-f]+ pending$' "$review"; then
    sed -E 's/^(decision row [0-9a-f]+) pending$/\1 accept/' "$review" > "$review.accepted"
    mv "$review.accepted" "$review"
    "$omega_cli" update --project "$gate" --resume
fi

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
"$omega_cli" --target "$target" --build-dir "$build_dir" "$gate/main.omg"
cli_identity=$(shasum -a 256 "$omega_cli" | awk '{print $1}')
artifact_identity=$(shasum -a 256 "$build_dir/omega-program" | awk '{print $1}')
printf '%s-gate cli_sha256=%s target=%s artifact_sha256=%s\n' \
    "$gate_name" "$cli_identity" "$target" "$artifact_identity"
python3 "$decoder" "$build_dir/omega-program"
