#!/usr/bin/env sh
# Focused native OMGLOW2 resolved-source -> explicit-root/call CKIR schema-2 gate.
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

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "resolved-to-CKIR2: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolved-to-CKIR2: skipped ($TOOL absent)"
    exit 0
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
DELTA_MANIFEST="$OMEGA_REPO_ROOT/bootstrap/delta/rust/Cargo.toml"
DELTA="$OMEGA_REPO_ROOT/bootstrap/delta/rust/target/debug/delta"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER1="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir.alp"
LOWERER2="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir2.alp"
FRAME1="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow.py"
FRAME2="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow2.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/role3_resolution_fixture.py"
LEGACY_FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/ckir2_call_reference.py"
SEMANTICS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v2_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"

for FILE in "$RESOLVER" "$LOWERER1" "$LOWERER2" "$FRAME1" "$FRAME2" \
  "$FIXTURE" "$LEGACY_FIXTURE" "$REFERENCE" "$SEMANTICS" "$LOWERMACHINE"; do
  [ -f "$FILE" ] || {
    echo "resolved-to-CKIR2: missing $FILE" >&2
    exit 1
  }
done

MACHINE_COUNT=$(awk '/^machine / { count += 1 } END { print count + 0 }' "$LOWERER2")
[ "$MACHINE_COUNT" -le 128 ] || {
  echo "resolved-to-CKIR2: lowerer exceeds Delta machine ceiling ($MACHINE_COUNT)" >&2
  exit 1
}

cargo build -q --manifest-path "$DELTA_MANIFEST"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER1" "$T/lowerer1" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER2" "$T/lowerer2" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null

"$T/lowermachine" < "$LOWERER2" > "$T/lowerer2.self.s"
clang -arch arm64 -o "$T/lowerer2.self" "$T/lowerer2.self.s"
codesign -f -s - "$T/lowerer2.self" >/dev/null 2>&1

python3 "$FIXTURE" build "$T/fixture"
python3 - "$T/cases" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, "bootstrap/omega-bootstrap/gates")
from role3_resolution_fixture import ROOT_SOURCE, SECOND_SOURCE, encode

out = Path(sys.argv[1]); out.mkdir()
out.joinpath("signature.omgc").write_bytes(
    encode(ROOT_SOURCE.replace("self.local(68)", "self.local(true)"), SECOND_SOURCE)
)
out.joinpath("nested.omgc").write_bytes(
    encode(ROOT_SOURCE.replace("self.local(68)", "self.local(self.cross(66))"), SECOND_SOURCE)
)
unit_root = """module app;
data Probe {}
machine Probe::run(&mut self) -> u8 {
    self.touch();
    70
}
machine Probe::touch(&mut self) {
}
"""
unit_second = """module app;
machine Probe::decoy(&self) -> u8 {
    7
}
"""
out.joinpath("unit.omgc").write_bytes(encode(unit_root, unit_second))
out.joinpath("unreachable-cycle.omgc").write_bytes(
    encode(ROOT_SOURCE, SECOND_SOURCE.replace("    7\n", "    self.decoy()\n"))
)
PY

run_expect() { # executable input status output label
  set +e
  "$1" < "$2" > "$4"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$3" ] || {
    echo "resolved-to-CKIR2 FAIL - $5 returned $ACTUAL, expected $3" >&2
    exit 1
  }
  if [ "$3" -ne 0 ] && [ -s "$4" ]; then
    echo "resolved-to-CKIR2 FAIL - $5 published bytes on rejection" >&2
    exit 1
  fi
}

CANONICAL="$T/fixture/valid.omgc"
run_expect "$T/resolver" "$CANONICAL" 0 "$T/canonical.witness" "canonical resolver"
python3 "$FIXTURE" check "$CANONICAL" "$T/canonical.witness" >/dev/null
python3 "$FRAME2" pack "$CANONICAL" "$T/canonical.witness" > "$T/canonical.low2"
python3 "$FRAME2" verify "$T/canonical.low2"
run_expect "$T/lowerer2" "$T/canonical.low2" 0 "$T/canonical.ckir2" "canonical lowerer"
run_expect "$T/lowerer2" "$T/canonical.low2" 0 "$T/repeat.ckir2" "canonical repeat"
run_expect "$T/lowerer2.self" "$T/canonical.low2" 0 "$T/canonical.self.ckir2" "self-built canonical lowerer"
cmp "$T/canonical.ckir2" "$T/repeat.ckir2"
cmp "$T/canonical.ckir2" "$T/canonical.self.ckir2"
python3 "$REFERENCE" emit "$T/expected.ckir2"
cmp "$T/expected.ckir2" "$T/canonical.ckir2"
[ "$(python3 "$REFERENCE" check "$T/canonical.ckir2")" = 70 ]
python3 "$SEMANTICS" validate "$T/canonical.ckir2" >/dev/null
[ "$(python3 "$SEMANTICS" run "$T/canonical.ckir2")" = 70 ]

# Nested argument calls use the same receiver-first, left-to-right rule.
run_expect "$T/resolver" "$T/cases/nested.omgc" 0 "$T/nested.witness" "nested resolver"
python3 "$FRAME2" pack "$T/cases/nested.omgc" "$T/nested.witness" > "$T/nested.low2"
run_expect "$T/lowerer2" "$T/nested.low2" 0 "$T/nested.ckir2" "nested lowerer"
python3 "$SEMANTICS" validate "$T/nested.ckir2" >/dev/null
[ "$(python3 "$SEMANTICS" run "$T/nested.ckir2")" = 70 ]

# Unit calls produce no value row and remain valid expression statements.
run_expect "$T/resolver" "$T/cases/unit.omgc" 0 "$T/unit.witness" "Unit-call resolver"
python3 "$FRAME2" pack "$T/cases/unit.omgc" "$T/unit.witness" > "$T/unit.low2"
run_expect "$T/lowerer2" "$T/unit.low2" 0 "$T/unit.ckir2" "Unit-call lowerer"
python3 "$SEMANTICS" validate "$T/unit.ckir2" >/dev/null
[ "$(python3 "$SEMANTICS" run "$T/unit.ckir2")" = 70 ]

# The call-free CKIR1 fixture retains every table/operation byte except the
# explicitly versioned schema-major field.  This also exercises field postfixes
# through the CKIR2 lookahead parser.
python3 "$LEGACY_FIXTURE" build "$T/legacy"
run_expect "$T/resolver" "$T/legacy/compilation-envelope.bin" 0 "$T/legacy.witness" "legacy resolver"
python3 "$FRAME1" pack "$T/legacy/compilation-envelope.bin" "$T/legacy.witness" > "$T/legacy.low1"
python3 "$FRAME2" pack "$T/legacy/compilation-envelope.bin" "$T/legacy.witness" > "$T/legacy.low2"
run_expect "$T/lowerer1" "$T/legacy.low1" 0 "$T/legacy.ckir1" "legacy CKIR1 lowerer"
run_expect "$T/lowerer2" "$T/legacy.low2" 0 "$T/legacy.ckir2" "legacy CKIR2 lowerer"
python3 - "$T/legacy.ckir1" "$T/legacy.expected2" <<'PY'
import sys
raw = bytearray(open(sys.argv[1], "rb").read())
raw[8] = 2
open(sys.argv[2], "wb").write(raw)
PY
cmp "$T/legacy.expected2" "$T/legacy.ckir2"

# The extra zero-parameter scalar decoy is accepted: entry is the exact
# OMGCOMP/OMGRSW1-selected run machine, not a global candidate inference.
python3 - "$T/canonical.witness" "$T/wrong-root.witness" "$T/resource.witness" <<'PY'
import struct, sys
raw = bytearray(open(sys.argv[1], "rb").read())
wrong = bytearray(raw); struct.pack_into("<I", wrong, 64, 3)
open(sys.argv[2], "wb").write(wrong)
resource = bytearray(raw); struct.pack_into("<I", resource, 36, 2049)
open(sys.argv[3], "wb").write(resource)
PY
python3 "$FRAME2" pack "$CANONICAL" "$T/wrong-root.witness" > "$T/wrong-root.low2"
run_expect "$T/lowerer2" "$T/wrong-root.low2" 251 "$T/wrong-root.out" "wrong explicit root"
run_expect "$T/lowerer2.self" "$T/wrong-root.low2" 251 "$T/wrong-root.self.out" "self-built wrong explicit root"
python3 "$FRAME2" pack "$CANONICAL" "$T/resource.witness" > "$T/resource.low2"
run_expect "$T/lowerer2" "$T/resource.low2" 252 "$T/resource.out" "witness resource"
run_expect "$T/lowerer2.self" "$T/resource.low2" 252 "$T/resource.self.out" "self-built witness resource"

for CASE in signature unreachable-cycle; do
  run_expect "$T/resolver" "$T/cases/$CASE.omgc" 0 "$T/$CASE.witness" "$CASE resolver"
  python3 "$FRAME2" pack "$T/cases/$CASE.omgc" "$T/$CASE.witness" > "$T/$CASE.low2"
  run_expect "$T/lowerer2" "$T/$CASE.low2" 251 "$T/$CASE.out" "$CASE lowerer"
done

# The two framing identities are intentionally non-interchangeable.
python3 "$FRAME1" pack "$CANONICAL" "$T/canonical.witness" > "$T/canonical.low1"
run_expect "$T/lowerer2" "$T/canonical.low1" 251 "$T/low1-to-2.out" "OMGLOW1 into CKIR2 lowerer"
run_expect "$T/lowerer1" "$T/canonical.low2" 251 "$T/low2-to-1.out" "OMGLOW2 into CKIR1 lowerer"

echo "resolved-to-CKIR2: $MACHINE_COUNT machines; native/self exact root/call artifact, DAG/nesting, call-free parity, signature/cycle, resource, and version separation passed"
