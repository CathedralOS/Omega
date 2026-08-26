#!/usr/bin/env sh
# Native/self resolver handoff for the bounded OMGCOMP2 -> OMGRSW6 relation.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRSW6 native/self handoff: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 cmp clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRSW6 native/self handoff: skipped ($TOOL absent)"
    exit 0
  }
done

REFERENCE=$GATE_DIR/omgrsw6_provider_resolution_reference.py
FIXTURE=$GATE_DIR/delta-provider-resolution-v6-fixture.py
RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp
LOWERMACHINE=$OMEGA_REPO_ROOT/bootstrap/delta/samples/lowermachine.alp
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

python3 -B "$REFERENCE" build "$T/reference" >/dev/null
python3 -B "$FIXTURE" build "$T/producer.omgc"
cmp "$T/producer.omgc" "$T/reference/canonical.omgc" >/dev/null

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
env DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
env DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null

python3 -B - "$RESOLVER" "$T/lowermachine" "$T/resolver.self" <<'PY'
from pathlib import Path
import re
import subprocess
import sys

source, lowermachine, output = map(Path, sys.argv[1:])
raw = re.sub(rb"//[^\n]*", b"", source.read_bytes())
raw = re.sub(rb"\s+", b" ", raw)
raw = re.sub(rb"\s*([^A-Za-z0-9_\s])\s*", rb"\1", raw)
assembly = output.with_suffix(".s")
with assembly.open("wb") as stream:
    result = subprocess.run([str(lowermachine)], input=raw, stdout=stream)
if result.returncode:
    raise SystemExit(f"OMGRSW6 self build: lowermachine status {result.returncode}")
subprocess.run(["clang", "-arch", "arm64", "-o", str(output), str(assembly)], check=True)
subprocess.run(["codesign", "-f", "-s", "-", str(output)], check=True,
               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
PY

run_case() { # executable label name status input
  EXECUTABLE=$1 LABEL=$2 NAME=$3 EXPECTED=$4 INPUT=$5
  set +e
  "$EXECUTABLE" < "$INPUT" > "$T/$NAME.$LABEL.out"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRSW6 native/self handoff: $NAME/$LABEL returned $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$T/$NAME.$LABEL.out" ]; then
    echo "OMGRSW6 native/self handoff: $NAME/$LABEL rejection published bytes" >&2
    exit 1
  fi
}

for LABEL in native self; do
  case "$LABEL" in
    native) EXECUTABLE=$T/resolver.native ;;
    self) EXECUTABLE=$T/resolver.self ;;
  esac
  run_case "$EXECUTABLE" "$LABEL" canonical 0 "$T/producer.omgc"
  cmp "$T/canonical.$LABEL.out" "$T/reference/canonical.omgrsw6" >/dev/null
  python3 -B "$FIXTURE" inspect "$T/producer.omgc" "$T/canonical.$LABEL.out" >/dev/null
  while IFS="$(printf '\t')" read -r NAME EXPECTED INPUT; do
    run_case "$EXECUTABLE" "$LABEL" "$NAME" "$EXPECTED" "$INPUT"
  done < "$T/reference/resolver-cases.tsv"
done
cmp "$T/canonical.native.out" "$T/canonical.self.out" >/dev/null

echo "OMGRSW6 native/self handoff: exact OMGCOMP2, 1064-byte OMGRSW6, semantic/resource controls, and no selected provider PASS"
