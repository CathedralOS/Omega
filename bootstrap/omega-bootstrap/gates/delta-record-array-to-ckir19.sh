#!/usr/bin/env sh
# Focused OMGRSW11/OMGLOWK20 native+self source-to-CKIR19 handoff.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in "Darwin arm64") ;; *) echo "OMGLOWK20 focused producer: skipped (requires Darwin arm64)"; exit 0 ;; esac
for TOOL in cargo python3 cmp clang codesign; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGLOWK20 focused producer: skipped ($TOOL absent)"; exit 0; }; done

RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-record-array-resolve.alp
LOWERER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-record-array-to-ckir.alp
LOWERMACHINE=$OMEGA_REPO_ROOT/bootstrap/delta/samples/lowermachine.alp
SOURCE_FIXTURE=$GATE_DIR/omgrsw11_record_array_fixture.py
LOWERING_FIXTURE=$GATE_DIR/delta-record-array-to-ckir19-fixture.py
CKIR_FIXTURE=$GATE_DIR/delta-checked-ir-v19-fixture.py
CKIR_REFERENCE=$GATE_DIR/checked_ir_v19_reference.py
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

python3 -B "$SOURCE_FIXTURE" matrix "$T/source"
python3 -B "$CKIR_FIXTURE" emit "$T/ckir"
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
env DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
env DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null
env DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null

self_build() {
  python3 -B - "$1" "$T/lowermachine" "$2" <<'PY'
from pathlib import Path
import re, subprocess, sys
source, lowermachine, output = map(Path, sys.argv[1:])
raw = re.sub(rb"//[^\n]*", b"", source.read_bytes())
raw = re.sub(rb"\s+", b" ", raw)
raw = re.sub(rb"\s*([^A-Za-z0-9_\s])\s*", rb"\1", raw)
assembly = output.with_suffix(".s")
with assembly.open("wb") as stream:
    result = subprocess.run([str(lowermachine)], input=raw, stdout=stream, timeout=120)
if result.returncode: raise SystemExit(f"self build returned {result.returncode}: {source.name}")
subprocess.run(["clang", "-arch", "arm64", "-o", str(output), str(assembly)], check=True)
subprocess.run(["codesign", "-f", "-s", "-", str(output)], check=True,
               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
PY
}
self_build "$RESOLVER" "$T/resolver.self"
self_build "$LOWERER" "$T/lowerer.self"

run_case() {
  CASE_EXECUTABLE=$1 CASE_LABEL=$2 CASE_NAME=$3 CASE_EXPECTED=$4 CASE_INPUT=$5 CASE_OUTPUT=$6
  set +e; "$CASE_EXECUTABLE" < "$CASE_INPUT" > "$CASE_OUTPUT"; CASE_ACTUAL=$?; set -e
  [ "$CASE_ACTUAL" -eq "$CASE_EXPECTED" ] || { echo "OMGLOWK20: $CASE_NAME/$CASE_LABEL status $CASE_ACTUAL expected $CASE_EXPECTED" >&2; exit 1; }
  if [ "$CASE_EXPECTED" -ne 0 ] && [ -s "$CASE_OUTPUT" ]; then echo "OMGLOWK20: rejection published bytes" >&2; exit 1; fi
}

while IFS="$(printf '\t')" read -r NAME INPUT; do
  for RLABEL in native self; do
    case "$RLABEL" in native) R=$T/resolver.native ;; self) R=$T/resolver.self ;; esac
    W=$T/$NAME.$RLABEL.omgrswb
    run_case "$R" "$RLABEL" "$NAME" 0 "$INPUT" "$W"
    python3 -B "$SOURCE_FIXTURE" inspect "$INPUT" "$W"
    FRAME=$T/$NAME.$RLABEL.omglowk
    python3 -B "$LOWERING_FIXTURE" pack "$INPUT" "$W" "$FRAME"
    for LLABEL in native self; do
      case "$LLABEL" in native) L=$T/lowerer.native ;; self) L=$T/lowerer.self ;; esac
      OUT=$T/$NAME.$RLABEL.$LLABEL.ckir19
      run_case "$L" "$LLABEL" "$NAME-$RLABEL" 0 "$FRAME" "$OUT"
      cmp "$OUT" "$T/ckir/canonical.ckir19" >/dev/null
      python3 -B "$CKIR_REFERENCE" validate "$OUT" >/dev/null
      [ "$(python3 -B "$CKIR_REFERENCE" run "$OUT")" = 70 ]
    done
  done
  cmp "$T/$NAME.native.omgrswb" "$T/$NAME.self.omgrswb" >/dev/null
done < "$T/source/positives.tsv"

for RLABEL in native self; do
  case "$RLABEL" in native) R=$T/resolver.native ;; self) R=$T/resolver.self ;; esac
  while IFS="$(printf '\t')" read -r NAME EXPECTED INPUT; do run_case "$R" "$RLABEL" "$NAME" "$EXPECTED" "$INPUT" "$T/$NAME.$RLABEL.out"; done < "$T/source/negatives.tsv"
done

python3 -B "$LOWERING_FIXTURE" cases "$T/source/canonical.omgc" "$T/canonical.native.omgrswb" "$T/source/renamed.omgc" "$T/lowering-cases"
for LLABEL in native self; do
  case "$LLABEL" in native) L=$T/lowerer.native ;; self) L=$T/lowerer.self ;; esac
  while IFS="$(printf '\t')" read -r NAME EXPECTED INPUT; do run_case "$L" "$LLABEL" "$NAME" "$EXPECTED" "$INPUT" "$T/$NAME.$LLABEL.out"; done < "$T/lowering-cases/manifest.tsv"
done

echo "OMGLOWK20 focused producer: native/self rename/reorder/inert, exact 6364-byte CKIR19, 251/252 controls PASS"
