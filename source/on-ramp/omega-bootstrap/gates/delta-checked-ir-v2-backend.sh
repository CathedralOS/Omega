#!/usr/bin/env sh
# Focused CKIR2 -> deterministic Linux x86-64 ELF backend gate.  The producer
# and lower-rooted refinement gates own source resolution and root selection;
# this gate begins at independently pinned checked-IR bytes.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "checked-IR-v2 backend: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v2 backend: skipped (compiler construction requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v2 backend: skipped ($TOOL absent)"
    exit 0
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v2-to-elf.alp"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/ckir2_call_reference.py"
SEMANTICS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v2_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v2_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for REQUIRED in "$BACKEND" "$REFERENCE" "$SEMANTICS" "$ELF_REFERENCE" "$LOWERMACHINE"; do
  [ -f "$REQUIRED" ] || {
    echo "checked-IR-v2 backend: required input absent: $REQUIRED" >&2
    exit 1
  }
done

PROCEDURES=$(grep -c '^machine ' "$BACKEND")
[ "$PROCEDURES" -lt 128 ] || {
  echo "checked-IR-v2 backend: $PROCEDURES procedures exceeds the Delta envelope" >&2
  exit 1
}

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_CKIR2_BACKEND_TEMP:-0}" = 1 ]; then
    echo "checked-IR-v2 backend: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null
"$T/lowermachine" < "$BACKEND" > "$T/backend.self.s"
clang -arch arm64 -o "$T/backend.self" "$T/backend.self.s"
codesign -f -s - "$T/backend.self" >/dev/null 2>&1

run_expect() { # executable input expected-status output label
  set +e
  "$1" < "$2" > "$4"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$3" ] || {
    echo "checked-IR-v2 backend FAIL - $5 returned $ACTUAL, expected $3" >&2
    exit 1
  }
  if [ "$3" -ne 0 ] && [ -s "$4" ]; then
    echo "checked-IR-v2 backend FAIL - $5 published bytes on rejection" >&2
    exit 1
  fi
}

reference_rejects() { # checked-IR label
  if python3 "$SEMANTICS" validate "$1" > "$T/reference.out" 2> "$T/reference.err"; then
    echo "checked-IR-v2 backend FAIL - reference accepted $2" >&2
    exit 1
  fi
}

python3 "$REFERENCE" emit "$T/canonical.ckir2"
[ "$(python3 "$REFERENCE" check "$T/canonical.ckir2")" = 70 ]
python3 "$SEMANTICS" validate "$T/canonical.ckir2" >/dev/null
[ "$(python3 "$SEMANTICS" run "$T/canonical.ckir2")" = 70 ]
run_expect "$T/backend.native" "$T/canonical.ckir2" 0 "$T/canonical.native.elf" "native canonical"
run_expect "$T/backend.self" "$T/canonical.ckir2" 0 "$T/canonical.self.elf" "self canonical"
cmp "$T/canonical.native.elf" "$T/canonical.self.elf"
python3 "$ELF_REFERENCE" mutation-sweep "$T/canonical.ckir2" "$T/canonical.native.elf" >/dev/null

# Build byte-local schema/call/root controls from the pinned table extents, and
# a maximum-operation semantic input whose place frame crosses the backend's
# 262144-byte live-stack resource cap.
python3 - "$T" <<'PY'
from pathlib import Path
import struct
import sys

sys.path.insert(0, "source/on-ramp/omega-bootstrap/gates")
import ckir2_call_reference as fixture

out = Path(sys.argv[1])
canonical = fixture.expected()
header = fixture.HEADER.unpack_from(canonical)
counts = header[7:17]
bases = []
cursor = fixture.HEADER.size
for count, row in zip(counts, fixture.ROWS):
    bases.append(cursor)
    cursor += count * row.size

def mutation(name, offset, value, form="I"):
    raw = bytearray(canonical)
    struct.pack_into("<" + form, raw, offset, value)
    out.joinpath(name + ".ckir2").write_bytes(raw)

operations = bases[7]
operands = bases[8]
machine_params = bases[4]
mutation("schema-major", 8, 1, "H")
mutation("entry-range", 16, 4)
mutation("entry-has-parameter", 16, 1)
mutation("call-callee", operations + 2 * fixture.ROWS[7].size + 32, 4)
mutation("call-arity", operations + 2 * fixture.ROWS[7].size + 28, 1)
mutation("call-imm1", operations + 2 * fixture.ROWS[7].size + 36, 1)
mutation("call-receiver", operands, 99)
mutation("call-argument-order", operands + 4, 3)
mutation("call-argument-type", machine_params + 12, 2)
mutation("call-result-type", operations + 2 * fixture.ROWS[7].size + 20, 2)
mutation("direct-cycle", operations + 4 * fixture.ROWS[7].size + 32, 1)
mutation("table-resource", 24 + 7 * 4, 32769)

unreachable_cycle = bytearray(canonical)
struct.pack_into("<I", unreachable_cycle, 16, 3)
struct.pack_into("<I", unreachable_cycle, operations + 4 * fixture.ROWS[7].size + 32, 1)
out.joinpath("unreachable-cycle.ckir2").write_bytes(unreachable_cycle)

# Machine 3 is an otherwise valid zero-parameter scalar decoy.  Selecting it is
# legal at this layer: CKIR2 consumes the selected root and does not infer one.
mutation("explicit-decoy-root", 16, 3)

n = 32768
types = [
    (0, 4, 0, 0, 0, 0, 0, 0), (1, 3, 0, 0, 0, 0, 0, 1),
    (2, 2, 0, 0, 0, 0, 0, 0x7fff_ffff), (3, 1, 0, 0, 0, 0, 0, 255),
]
records = [(0, 0, 0, 0, 0)]
machines = [(0, 0, 2, 0, 0, 3, 0, 0, 0, 1, 0)]
blocks = [(0, 0, 2, 0, 0, 0, 0, 0, n, 0)]
operations_rows = [
    (i, 0, 0, 2, 2, 0, i, 0, 0, 0, 0, 0) for i in range(n - 1)
]
operations_rows.append((n - 1, 0, 0, 1, 1, 0, 0, 3, 0, 0, 7, 0))
terminators = [(0, 0, 0, 4, 0, 0, 0, 0xffffffff, 0, 0, 0xffffffff, 0, 0)]
tables = (
    types, records, [], machines, [], blocks, [], operations_rows, [], terminators,
)
payload = b"".join(
    row_type.pack(*row)
    for table, row_type in zip(tables, fixture.ROWS)
    for row in table
)
resource_counts = tuple(len(table) for table in tables)
out.joinpath("live-stack-resource.ckir2").write_bytes(
    fixture.HEADER.pack(
        b"OMGCKIR\0", 2, 0, 1, 1, 0, fixture.HEADER.size + len(payload),
        *resource_counts, 1, n - 1,
    ) + payload
)
PY

# An explicit alternate root remains deterministic and is reconstructed exactly.
[ "$(python3 "$SEMANTICS" run "$T/explicit-decoy-root.ckir2")" = 7 ]
for BACKEND_EXE in "$T/backend.native" "$T/backend.self"; do
  run_expect "$BACKEND_EXE" "$T/explicit-decoy-root.ckir2" 0 "$T/decoy.elf" "explicit decoy root"
done
python3 "$ELF_REFERENCE" check "$T/explicit-decoy-root.ckir2" "$T/decoy.elf" >/dev/null

for CASE in schema-major entry-range entry-has-parameter call-callee call-arity \
  call-imm1 call-receiver call-argument-order call-argument-type call-result-type \
  direct-cycle unreachable-cycle; do
  reference_rejects "$T/$CASE.ckir2" "$CASE"
  run_expect "$T/backend.native" "$T/$CASE.ckir2" 251 "$T/$CASE.native.out" "$CASE native"
  run_expect "$T/backend.self" "$T/$CASE.ckir2" 251 "$T/$CASE.self.out" "$CASE self"
done

reference_rejects "$T/table-resource.ckir2" "table resource"
run_expect "$T/backend.native" "$T/table-resource.ckir2" 252 "$T/table-resource.native.out" "table resource native"
run_expect "$T/backend.self" "$T/table-resource.ckir2" 252 "$T/table-resource.self.out" "table resource self"

# The checked IR is structurally valid; only backend frame construction should
# classify this as resource exhaustion and publish no partial artifact.
python3 "$SEMANTICS" validate "$T/live-stack-resource.ckir2" >/dev/null
run_expect "$T/backend.native" "$T/live-stack-resource.ckir2" 252 "$T/live-stack.native.out" "live stack native"
run_expect "$T/backend.self" "$T/live-stack-resource.ckir2" 252 "$T/live-stack.self.out" "live stack self"

echo "checked-IR-v2 backend: $PROCEDURES procedures; native/self call DAG, explicit root, exact argument type, cycle, resource, and exact ELF controls passed"
