#!/usr/bin/env sh
# Actual native/self OMGCOMP3 -> OMGRSW9 -> CKIR17 bytes joined to R1--R5.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN20 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 cmp clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN20 same-frame composite: skipped ($TOOL absent)"
    exit 0
  }
done

G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
REFERENCE=$G/omgrsw9_provider_plan_reference.py
FIXTURE=$G/omgrsw9_provider_plan_fixture.py
CKIR_FIXTURE=$G/delta-checked-ir-v17-fixture.py
CKIR_REFERENCE=$G/checked_ir_v17_reference.py
RESOLVER=$C/omega-bootstrap-provider-plan.alp
LOWERER=$C/omega-bootstrap-provider-plan-to-ckir.alp
LOWERMACHINE=$OMEGA_REPO_ROOT/source/delta/samples/lowermachine.alp
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

python3 -B "$REFERENCE" build "$T/reference" >/dev/null
python3 -B "$FIXTURE" build "$T/producer.omgc"
python3 -B "$CKIR_FIXTURE" emit "$T/ckir-reference" >/dev/null
cmp "$T/producer.omgc" "$T/reference/canonical.omgc" >/dev/null

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
env DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
env DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null
env DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null

python3 -B - "$T/lowermachine" "$T" "$RESOLVER" "$LOWERER" <<'PY'
from pathlib import Path
import re
import subprocess
import sys

lowermachine, output = map(Path, sys.argv[1:3])
for name, source in (("resolver", Path(sys.argv[3])),
                     ("lowerer", Path(sys.argv[4]))):
    raw = re.sub(rb"//[^\n]*", b"", source.read_bytes())
    raw = re.sub(rb"\s+", b" ", raw)
    raw = re.sub(rb"\s*([^A-Za-z0-9_\s])\s*", rb"\1", raw)
    assembly = output / f"{name}.self.s"
    executable = output / f"{name}.self"
    with assembly.open("wb") as stream:
        result = subprocess.run([str(lowermachine)], input=raw, stdout=stream)
    if result.returncode:
        raise SystemExit(f"OMGRFN20 {name} self build status {result.returncode}")
    subprocess.run(["clang", "-arch", "arm64", "-o", str(executable),
                    str(assembly)], check=True)
    subprocess.run(["codesign", "-f", "-s", "-", str(executable)], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
PY

check_mode() {
  MODE=$1
  "$T/resolver.$MODE" < "$T/producer.omgc" > "$T/actual.$MODE.omgrsw9"
  cmp "$T/actual.$MODE.omgrsw9" "$T/reference/canonical.omgrsw9" >/dev/null
  python3 -B "$HERE/omgrfn20-production.py" lowering \
    "$T/producer.omgc" "$T/actual.$MODE.omgrsw9" "$T/$MODE.low18"
  "$T/lowerer.$MODE" < "$T/$MODE.low18" > "$T/actual.$MODE.ckir17"
  cmp "$T/actual.$MODE.ckir17" "$T/ckir-reference/canonical.ckir17" >/dev/null
  python3 -B "$CKIR_REFERENCE" validate "$T/actual.$MODE.ckir17" >/dev/null
  python3 -B "$HERE/omgrfn20-production.py" refinement \
    "$T/producer.omgc" "$T/actual.$MODE.omgrsw9" \
    "$T/actual.$MODE.ckir17" "$T/$MODE.rfn"
  for OWNER in r1 r2 r3 r4 r5; do
    python3 -B "$HERE/omgrfn20-$OWNER.py" < "$T/$MODE.rfn" > "$T/$MODE-$OWNER.out"
    [ ! -s "$T/$MODE-$OWNER.out" ] || exit 1
  done
}

check_mode native
check_mode self
cmp "$T/actual.native.omgrsw9" "$T/actual.self.omgrsw9" >/dev/null
cmp "$T/actual.native.ckir17" "$T/actual.self.ckir17" >/dev/null
cmp "$T/native.rfn" "$T/self.rfn" >/dev/null

echo "OMGRFN20 same-frame composite: actual native/self OMGCOMP3 -> exact OMGRSW9 -> exact CKIR17 bytes satisfy R1-R5 PASS"
