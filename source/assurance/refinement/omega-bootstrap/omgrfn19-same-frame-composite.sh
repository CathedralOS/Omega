#!/bin/sh
# Actual native/self focused-producer bytes joined to every OMGRFN19 owner.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN19 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 cmp clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN19 same-frame composite: skipped ($TOOL absent)"
    exit 0
  }
done

REFERENCE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/omgrsw9_provider_plan_reference.py
FIXTURE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/omgrsw9_provider_plan_fixture.py
PRODUCER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-provider-plan.alp
LOWERMACHINE=$OMEGA_REPO_ROOT/bootstrap/delta/samples/lowermachine.alp
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

python3 -B "$REFERENCE" build "$T/reference" >/dev/null
python3 -B "$FIXTURE" build "$T/producer.omgc"
cmp "$T/producer.omgc" "$T/reference/canonical.omgc" >/dev/null

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
env DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER" "$T/producer.native" >/dev/null
env DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null

check_mode() {
  MODE=$1
  "$T/producer.$MODE" < "$T/producer.omgc" > "$T/actual.$MODE.omgrsw9"
  cmp "$T/actual.$MODE.omgrsw9" "$T/reference/canonical.omgrsw9" >/dev/null
  python3 -B "$HERE/omgrfn19_bundle.py" \
    "$T/producer.omgc" "$T/actual.$MODE.omgrsw9" > "$T/$MODE.rfn"
  for OWNER in r1 r2 r3 r4 r5; do
    python3 -B "$HERE/omgrfn19-$OWNER.py" < "$T/$MODE.rfn" > "$T/$MODE-$OWNER.out"
    [ ! -s "$T/$MODE-$OWNER.out" ] || exit 1
  done
}

# Preserve the reachable native half even when the larger self-host lowering is
# beyond the current lowermachine capacity.  The composite does not pass until
# the self half below reaches and reproduces these exact bytes.
check_mode native

python3 -B - "$PRODUCER" "$T/lowermachine" "$T/producer.self" <<'PY'
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
    raise SystemExit(f"OMGRFN19 self build: lowermachine status {result.returncode}")
subprocess.run(["clang", "-arch", "arm64", "-o", str(output), str(assembly)], check=True)
subprocess.run(["codesign", "-f", "-s", "-", str(output)], check=True,
               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
PY

check_mode self
cmp "$T/actual.native.omgrsw9" "$T/actual.self.omgrsw9" >/dev/null
cmp "$T/native.rfn" "$T/self.rfn" >/dev/null

echo "OMGRFN19 same-frame composite: actual native/self OMGCOMP3 -> exact OMGRSW9 bytes satisfy R1-R5 PASS"
