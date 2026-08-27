#!/usr/bin/env sh
# Focused OMGCOMP3 + OMGRSW9 -> CKIR17 native/self producer handoff.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGLOWI18 focused producer: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 cmp clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGLOWI18 focused producer: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-provider-plan.alp
LOWERER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-provider-plan-to-ckir.alp
LOWERMACHINE=$OMEGA_REPO_ROOT/source/delta/samples/lowermachine.alp
RESOLUTION_REFERENCE=$GATE_DIR/omgrsw9_provider_plan_reference.py
RESOLUTION_FIXTURE=$GATE_DIR/omgrsw9_provider_plan_fixture.py
LOWERING_FIXTURE=$GATE_DIR/delta-provider-plan-to-ckir17-fixture.py
CKIR_FIXTURE=$GATE_DIR/delta-checked-ir-v17-fixture.py
CKIR_REFERENCE=$GATE_DIR/checked_ir_v17_reference.py
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

python3 -B "$RESOLUTION_REFERENCE" build "$T/resolution-reference" >/dev/null
python3 -B "$RESOLUTION_FIXTURE" build "$T/canonical.omgc"
cmp "$T/canonical.omgc" "$T/resolution-reference/canonical.omgc" >/dev/null
python3 -B "$CKIR_FIXTURE" emit "$T/ckir-reference"

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
env DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
env DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null
env DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null

self_build() { # source output
  python3 -B - "$1" "$T/lowermachine" "$2" <<'PY'
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
    raise SystemExit(f"self build returned {result.returncode}: {source.name}")
subprocess.run(["clang", "-arch", "arm64", "-o", str(output), str(assembly)],
               check=True)
subprocess.run(["codesign", "-f", "-s", "-", str(output)], check=True,
               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
PY
}

self_build "$RESOLVER" "$T/resolver.self"
self_build "$LOWERER" "$T/lowerer.self"

"$T/resolver.native" < "$T/canonical.omgc" > "$T/native.omgrsw9"
"$T/resolver.self" < "$T/canonical.omgc" > "$T/self.omgrsw9"
cmp "$T/native.omgrsw9" "$T/self.omgrsw9" >/dev/null
cmp "$T/native.omgrsw9" "$T/resolution-reference/canonical.omgrsw9" >/dev/null
python3 -B "$RESOLUTION_REFERENCE" check "$T/canonical.omgc" "$T/native.omgrsw9" >/dev/null

python3 -B "$LOWERING_FIXTURE" pack "$T/canonical.omgc" "$T/native.omgrsw9" "$T/native.omglowi"
python3 -B "$LOWERING_FIXTURE" pack "$T/canonical.omgc" "$T/self.omgrsw9" "$T/self.omglowi"
python3 -B "$LOWERING_FIXTURE" cases "$T/canonical.omgc" "$T/native.omgrsw9" "$T/cases"

run_case() { # executable label name status input
  EXECUTABLE=$1 LABEL=$2 NAME=$3 EXPECTED=$4 INPUT=$5
  set +e
  "$EXECUTABLE" < "$INPUT" > "$T/$NAME.$LABEL.out"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGLOWI18 focused producer: $NAME/$LABEL returned $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$T/$NAME.$LABEL.out" ]; then
    echo "OMGLOWI18 focused producer: $NAME/$LABEL rejection published bytes" >&2
    exit 1
  fi
}

# Cross native/self resolver and lowerer products; both paths must publish the
# same independently frozen checked image.
run_case "$T/lowerer.native" native native-resolution 0 "$T/native.omglowi"
run_case "$T/lowerer.native" native self-resolution 0 "$T/self.omglowi"
run_case "$T/lowerer.self" self native-resolution 0 "$T/native.omglowi"
run_case "$T/lowerer.self" self self-resolution 0 "$T/self.omglowi"
for OUTPUT in \
  "$T/native-resolution.native.out" "$T/self-resolution.native.out" \
  "$T/native-resolution.self.out" "$T/self-resolution.self.out"; do
  cmp "$OUTPUT" "$T/ckir-reference/canonical.ckir17" >/dev/null
  python3 -B "$CKIR_REFERENCE" validate "$OUTPUT" >/dev/null
done

for LABEL in native self; do
  case "$LABEL" in
    native) EXECUTABLE=$T/lowerer.native ;;
    self) EXECUTABLE=$T/lowerer.self ;;
  esac
  while IFS="$(printf '\t')" read -r NAME EXPECTED INPUT; do
    run_case "$EXECUTABLE" "$LABEL" "$NAME" "$EXPECTED" "$INPUT"
  done < "$T/cases/manifest.tsv"
done

echo "OMGLOWI18 focused producer: native/self/cross-pair exact 2432-byte CKIR17, 251/252 controls PASS"
