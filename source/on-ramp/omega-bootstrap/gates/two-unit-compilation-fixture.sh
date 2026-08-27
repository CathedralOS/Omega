#!/usr/bin/env sh
# Fixture-only generation gate. This checks deterministic inputs and pinned
# digests; it does not claim source-resolution or compilation acceptance.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
FIXTURE_TOOL="$GATE_DIR/two_unit_compilation_fixture.py"
NEGATIVE_TOOL="$GATE_DIR/two_unit_compilation_negatives.py"
[ -f "$FIXTURE_TOOL" ] || {
  echo "two-unit compilation fixture: generator absent" >&2
  exit 1
}
[ -f "$NEGATIVE_TOOL" ] || {
  echo "two-unit compilation fixture: negative generator absent" >&2
  exit 1
}
command -v python3 >/dev/null 2>&1 || {
  echo "two-unit compilation fixture: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

# The canonical build rechecks its source, envelope, and reference-bundle pins.
python3 "$FIXTURE_TOOL" build "$T/canonical"
python3 "$NEGATIVE_TOOL" build "$T/negative-a"
python3 "$NEGATIVE_TOOL" check "$T/negative-a"
python3 "$NEGATIVE_TOOL" build "$T/negative-b"
diff -ru "$T/negative-a" "$T/negative-b"

echo "two-unit compilation fixture: canonical pins and deterministic negative inventory passed"
