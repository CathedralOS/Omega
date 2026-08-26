#!/usr/bin/env sh
# Independent OMGCOMP3 -> OMGRSW9 provider-plan reference gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

REFERENCE=$GATE_DIR/omgrsw9_provider_plan_reference.py
PRODUCER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-provider-plan.alp
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

command -v python3 >/dev/null 2>&1 || {
  echo "OMGRSW9 independent reference: python3 required" >&2
  exit 1
}
python3 -B "$REFERENCE" build "$T/reference"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRSW9 independent reference: producer comparison skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRSW9 independent reference: producer comparison skipped ($TOOL absent)"
    exit 0
  }
done

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
env DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER" "$T/producer" >/dev/null

run_case() { # name expected-status input
  NAME=$1 EXPECTED=$2 INPUT=$3
  set +e
  "$T/producer" < "$INPUT" > "$T/$NAME.stdout"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRSW9 independent reference: $NAME status $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$T/$NAME.stdout" ]; then
    echo "OMGRSW9 independent reference: $NAME rejection published bytes" >&2
    exit 1
  fi
}

run_case canonical 0 "$T/reference/canonical.omgc"
cmp "$T/canonical.stdout" "$T/reference/canonical.omgrsw9" >/dev/null || {
  echo "OMGRSW9 independent reference: focused producer differs from independent exact bytes" >&2
  exit 1
}
python3 -B "$REFERENCE" check "$T/reference/canonical.omgc" "$T/canonical.stdout" >/dev/null

while IFS="$(printf '\t')" read -r NAME INPUT EXPECTED_OUTPUT; do
  run_case "$NAME" 0 "$INPUT"
  cmp "$T/$NAME.stdout" "$EXPECTED_OUTPUT" >/dev/null || {
    echo "OMGRSW9 independent reference: $NAME differs from independent bytes" >&2
    exit 1
  }
  python3 -B "$REFERENCE" check "$INPUT" "$T/$NAME.stdout" >/dev/null
done < "$T/reference/positive-cases.tsv"

while IFS="$(printf '\t')" read -r NAME EXPECTED INPUT; do
  run_case "$NAME" "$EXPECTED" "$INPUT"
done < "$T/reference/resolver-cases.tsv"

echo "OMGRSW9 independent reference: exact source-derived plan, mutation, resource, EOF, and no-publication controls PASS"
