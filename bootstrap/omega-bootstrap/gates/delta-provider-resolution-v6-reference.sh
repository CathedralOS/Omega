#!/usr/bin/env sh
# Independent OMGCOMP2 -> OMGRSW6 provider-resolution reference gate.
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

REFERENCE="$GATE_DIR/omgrsw6_provider_resolution_reference.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

command -v python3 >/dev/null 2>&1 || {
  echo "OMGRSW6 independent reference: python3 required" >&2
  exit 1
}
python3 -B "$REFERENCE" build "$T/reference"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRSW6 independent reference: producer comparison skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRSW6 independent reference: producer comparison skipped ($TOOL absent)"
    exit 0
  }
done

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
env DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null

run_case() { # name expected-status input
  NAME=$1 EXPECTED=$2 INPUT=$3
  set +e
  "$T/resolver" < "$INPUT" > "$T/$NAME.stdout"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRSW6 independent reference: $NAME status $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$T/$NAME.stdout" ]; then
    echo "OMGRSW6 independent reference: $NAME rejection published bytes" >&2
    exit 1
  fi
}

run_case canonical 0 "$T/reference/canonical.omgc"
cmp "$T/canonical.stdout" "$T/reference/canonical.omgrsw6" >/dev/null || {
  echo "OMGRSW6 independent reference: resolver differs from independent exact bytes" >&2
  exit 1
}
python3 -B "$REFERENCE" check "$T/reference/canonical.omgc" "$T/canonical.stdout" >/dev/null

while IFS="$(printf '\t')" read -r NAME EXPECTED INPUT; do
  run_case "$NAME" "$EXPECTED" "$INPUT"
done < "$T/reference/resolver-cases.tsv"

echo "OMGRSW6 independent reference: exact tables/semantics, mutation, resource, EOF, and no-publication controls PASS"
