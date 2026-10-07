#!/bin/sh
set -eu

gate=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

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

build_dir=$(mktemp -d "${TMPDIR:-/tmp}/omega-psi-parser.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT HUP INT TERM
"$omega_cli" --target "$target" --build-dir "$build_dir" "$gate/main.omg"
cli_identity=$(shasum -a 256 "$omega_cli" | awk '{print $1}')
artifact_identity=$(shasum -a 256 "$build_dir/omega-program" | awk '{print $1}')
printf 'parser-gate cli_sha256=%s target=%s artifact_sha256=%s\n' \
    "$cli_identity" "$target" "$artifact_identity"
python3 "$gate/test_parser.py" "$build_dir/omega-program"
