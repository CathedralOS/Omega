#!/bin/sh
# Shared driver for the product Psi gates. Each gate is its own package under
# tests/psi/<gate>/ with a gate-owned Omega harness and a semantic-free Python
# decoder: `gate.sh <gate> <decoder.py>` reviews the package, builds the
# harness natively once and runs the decoder's cases against that artifact.
set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: gate.sh <gate-directory-name> <decoder.py>" >&2
    exit 2
fi
gate=$(CDPATH= cd -- "$(dirname -- "$0")/$1" && pwd)
gate_name=$1
decoder=$2

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
# (tools/sample_native_gate.py follows the same rule).
# Exit 3 reports pending review decisions, which the next step accepts.
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

build_dir=$(mktemp -d "${TMPDIR:-/tmp}/omega-psi-$gate_name.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM
"$omega_cli" --target "$target" --build-dir "$build_dir" "$gate/main.omg"
cli_identity=$(shasum -a 256 "$omega_cli" | awk '{print $1}')
artifact_identity=$(shasum -a 256 "$build_dir/omega-program" | awk '{print $1}')
printf '%s-gate cli_sha256=%s target=%s artifact_sha256=%s\n' \
    "$gate_name" "$cli_identity" "$target" "$artifact_identity"
python3 "$gate/$decoder" "$build_dir/omega-program"
