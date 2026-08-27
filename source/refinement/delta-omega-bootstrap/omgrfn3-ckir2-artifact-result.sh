#!/usr/bin/env sh
# OMGRFN3 layer-5a complete CKIR2 structure and selected-result evidence.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN3 layer 5a: skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v python3 >/dev/null 2>&1 || {
  echo "OMGRFN3 layer 5a: skipped (python3 absent)"
  exit 0
}

ENVELOPE="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3-component-envelope.beta"
ARTIFACT="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir2-refinement-artifact.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3_bundle.py"
OLD_PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2_bundle.py"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/ckir2_call_reference.py"
for REQUIRED in "$ENVELOPE" "$ARTIFACT" "$PACKER" "$OLD_PACKER" "$REFERENCE"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN3 layer 5a: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(date +%s)
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null
cp "$ENVELOPE" "$T/check.beta"
sed '/^proc main()/,$d' "$ARTIFACT" >> "$T/check.beta"
printf '%s\n' \
  '' \
  'proc main() {' \
  '    let status = omgrfn3_component_read()' \
  '    state envelope { to done when (status != 0)  status = ckir_refinement_artifact_check()  to done }' \
  '    state done { return status }' \
  '}' >> "$T/check.beta"
PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/check.beta")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN3 layer 5a: checker exceeds 128 procedures ($PROCEDURES)" >&2
  exit 1
}
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

printf opaque-omgcomp > "$T/omgcomp"
printf opaque-witness > "$T/witness"
printf opaque-elf > "$T/elf"
: > "$T/empty"

pack() ( # name ckir result
  PACK_NAME=$1
  PACK_CKIR=$2
  PACK_RESULT=$3
  python3 "$PACKER" "$T/omgcomp" "$T/witness" "$PACK_CKIR" "$T/elf" \
    --result "$PACK_RESULT" > "$T/$PACK_NAME.rfn"
)

observe() { # expected input label
  EXPECTED=$1
  INPUT=$2
  LABEL=$3
  set +e
  "$T/check" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN3 layer 5a: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN3 layer 5a: $LABEL published stdout" >&2
    exit 1
  }
}

python3 "$REFERENCE" emit "$T/canonical.ckir"
pack canonical "$T/canonical.ckir" 70
observe 0 "$T/canonical.rfn" "canonical three-call-machine DAG result"
pack wrong-claim "$T/canonical.ckir" 71
observe 251 "$T/wrong-claim.rfn" "wrong full-width selected result"

python3 - "$T/canonical.ckir" "$T/library.ckir" <<'PY'
from pathlib import Path
import struct
import sys
raw = bytearray(Path(sys.argv[1]).read_bytes())
struct.pack_into("<H", raw, 14, 0)
struct.pack_into("<I", raw, 16, 0xFFFF_FFFF)
Path(sys.argv[2]).write_bytes(raw)
PY
python3 "$PACKER" "$T/omgcomp" "$T/witness" "$T/library.ckir" "$T/empty" \
  --library > "$T/library.rfn"
observe 0 "$T/library.rfn" "schema-2 library with no selected root"

# Byte-local controls isolate schema identity, explicit-root semantics, and
# every Call relation without importing the Python reference verdict.
python3 - "$T/canonical.ckir" "$T/mutations" <<'PY'
from pathlib import Path
import struct
import sys

raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2]); out.mkdir()
header = struct.unpack_from("<8sHHHH14I", raw)
counts = header[7:]
sizes = (24, 20, 16, 36, 20, 32, 20, 40, 4, 44)
bases = []
cursor = 72
for count, size in zip(counts[:10], sizes):
    bases.append(cursor)
    cursor += count * size

operations = bases[7]
operands = bases[8]
machines = bases[3]
machine_parameters = bases[4]
blocks = bases[5]
calls = []
for op_id in range(counts[7]):
    row = operations + op_id * 40
    if raw[row + 12] == 10:
        calls.append((op_id, row))
assert len(calls) == 2
first_id, first = calls[0]
callee = struct.unpack_from("<I", raw, first + 32)[0]
operand_start = struct.unpack_from("<I", raw, first + 24)[0]
param_start = struct.unpack_from("<I", raw, machines + callee * 36 + 16)[0]

def put(name, contents):
    out.joinpath(name + ".ckir").write_bytes(contents)

def changed(name, at, value):
    data = bytearray(raw); data[at] = value; put(name, data)

schema = bytearray(raw); struct.pack_into("<H", schema, 8, 1); put("schema1", schema)
root = bytearray(raw); struct.pack_into("<I", root, 16, 99); put("root-range", root)
target = bytearray(raw); struct.pack_into("<I", target, first + 32, 99); put("call-target", target)
imm1 = bytearray(raw); struct.pack_into("<I", imm1, first + 36, 1); put("call-imm1", imm1)
receiver = bytearray(raw); struct.pack_into("<I", receiver, operands + operand_start * 4, 99); put("call-receiver", receiver)
argument = bytearray(raw); struct.pack_into("<I", argument, operands + (operand_start + 1) * 4, 99); put("call-argument", argument)
arg_type = bytearray(raw); struct.pack_into("<I", arg_type, machine_parameters + param_start * 20 + 12, 1); put("call-argument-type", arg_type)
result_type = bytearray(raw); struct.pack_into("<I", result_type, first + 20, 1); put("call-result-type", result_type)
result_shape = bytearray(raw); result_shape[first + 13] = 0; put("call-result-shape", result_shape)
shared = bytearray(raw)
shared[machines + 8] = 1
shared[blocks + 8] = 1
put("mutable-call-shared-receiver", shared)
exhausted = bytearray(raw); struct.pack_into("<I", exhausted, 24, 8193); put("type-count-exhausted", exhausted)

# CKIR2 carries the exact entry ID.  Selecting the unrelated decoy is valid
# despite the original root remaining another zero-parameter scalar machine.
decoy = bytearray(raw); struct.pack_into("<I", decoy, 16, 3); put("selected-decoy", decoy)
PY

for CASE in schema1 root-range call-target call-imm1 call-receiver call-argument \
  call-argument-type call-result-type call-result-shape \
  mutable-call-shared-receiver; do
  pack "$CASE" "$T/mutations/$CASE.ckir" 70
  observe 251 "$T/$CASE.rfn" "$CASE"
done
pack type-count-exhausted "$T/mutations/type-count-exhausted.ckir" 70
observe 252 "$T/type-count-exhausted.rfn" "published type-table exhaustion"
pack selected-decoy "$T/mutations/selected-decoy.ckir" 7
observe 0 "$T/selected-decoy.rfn" "explicit decoy root without candidate cardinality"

# Independently constructed schema-2 controls cover Unit calls, complete-graph
# cycles, and the evaluator's bounded active-call storage.  These builders emit
# raw rows only; the persisted-Beta checker remains the accepting authority.
python3 - "$T/generated" <<'PY'
from pathlib import Path
import struct
import sys

out = Path(sys.argv[1]); out.mkdir()
NO = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH14I")
ROWS = (
    struct.Struct("<IBBHIIII"), struct.Struct("<IIIIB3x"),
    struct.Struct("<IIII"), struct.Struct("<IIBBHIIIIII"),
    struct.Struct("<IIIII"), struct.Struct("<IIBBHIIIII"),
    struct.Struct("<IIIII"), struct.Struct("<IIIBBHIIIIII"),
    struct.Struct("<I"), struct.Struct("<IIIBBHIIIIIII"),
)
TYPES = [
    (0, 4, 0, 0, 0, 0, 0, 0),
    (1, 3, 0, 0, 0, 0, 0, 1),
    (2, 2, 0, 0, 0, 0, 0, 0x7FFF_FFFF),
    (3, 1, 0, 0, 0, 0, 0, 255),
]
RECORDS = [(0, 0, 0, 0, 0)]

def encode(name, machines, blocks, operations, operands, terms, values, places):
    # These controls have no CFG edge arguments, so every terminator's two
    # empty spans begin at the end of the complete operation-operand partition.
    normalized_terms = []
    for term in terms:
        row = list(term)
        row[8] = len(operands)
        row[11] = len(operands)
        normalized_terms.append(tuple(row))
    tables = (TYPES, RECORDS, [], machines, [], blocks, [], operations,
              [(value,) for value in operands], normalized_terms)
    payload = b"".join(row_type.pack(*row) for table, row_type in zip(tables, ROWS) for row in table)
    counts = tuple(len(table) for table in tables)
    raw = HEADER.pack(b"OMGCKIR\0", 2, 0, 1, 1, 0,
                      HEADER.size + len(payload), *counts, values, places) + payload
    out.joinpath(name + ".ckir").write_bytes(raw)

def chain(name, machine_count, cycle=False):
    machines=[]; blocks=[]; operations=[]; operands=[]; terms=[]
    value_id=0; place_id=0
    for machine in range(machine_count):
        op_start=len(operations)
        if machine + 1 < machine_count:
            operations.append((len(operations),machine,machine,2,2,0,place_id,0,len(operands),0,0,0))
            target = machine if cycle and machine == 0 else machine + 1
            operations.append((len(operations),machine,machine,10,1,0,value_id,3,len(operands),1,target,0))
            operands.append(place_id)
            result=value_id; value_id+=1; place_id+=1
        else:
            operations.append((len(operations),machine,machine,1,1,0,value_id,3,len(operands),0,70,0))
            result=value_id; value_id+=1
        blocks.append((machine,machine,2,0,0,0,0,op_start,len(operations)-op_start,machine))
        machines.append((machine,0,2,0,0,3,0,0,machine,1,machine))
        terms.append((machine,machine,machine,4,0,0,result,NO,len(operands),0,NO,len(operands),0))
    encode(name,machines,blocks,operations,operands,terms,value_id,place_id)

def unit_call():
    machines=[(0,0,2,0,0,3,0,0,0,1,0),(1,0,1,0,0,NO,0,0,1,1,1)]
    blocks=[(0,0,2,0,0,0,0,0,3,0),(1,1,1,0,0,0,0,3,0,1)]
    operations=[
        (0,0,0,2,2,0,0,0,0,0,0,0),
        (1,0,0,10,0,0,NO,NO,0,1,1,0),
        (2,0,0,1,1,0,0,3,1,0,70,0),
    ]
    operands=[0]
    terms=[(0,0,0,4,0,0,0,NO,1,0,NO,1,0),(1,1,1,3,0,0,NO,NO,1,0,NO,1,0)]
    encode("unit-call",machines,blocks,operations,operands,terms,1,1)

def unreachable_cycle():
    machines=[(0,0,2,0,0,3,0,0,0,1,0),(1,0,2,0,0,3,0,0,1,1,1)]
    blocks=[(0,0,2,0,0,0,0,0,1,0),(1,1,2,0,0,0,0,1,2,1)]
    operations=[
        (0,0,0,1,1,0,0,3,0,0,70,0),
        (1,1,1,2,2,0,0,0,0,0,0,0),
        (2,1,1,10,1,0,1,3,0,1,1,0),
    ]
    operands=[0]
    terms=[(0,0,0,4,0,0,0,NO,1,0,NO,1,0),(1,1,1,4,0,0,1,NO,1,0,NO,1,0)]
    encode("unreachable-cycle",machines,blocks,operations,operands,terms,2,1)

def runtime_trap():
    machines=[(0,0,2,0,0,3,0,0,0,1,0)]
    blocks=[(0,0,2,0,0,0,0,0,3,0)]
    operations=[
        (0,0,0,1,1,0,0,3,0,0,250,0),
        (1,0,0,1,1,0,1,3,0,0,10,0),
        (2,0,0,8,1,0,2,3,0,2,0,0),
    ]
    operands=[0,1]
    terms=[(0,0,0,4,0,0,2,NO,2,0,NO,2,0)]
    encode("runtime-trap",machines,blocks,operations,operands,terms,3,0)

unit_call()
chain("direct-cycle",2,cycle=True)
unreachable_cycle()
runtime_trap()
chain("depth-boundary",65)
chain("depth-exhausted",66)
PY

pack unit-call "$T/generated/unit-call.ckir" 70
observe 0 "$T/unit-call.rfn" "valid Unit call and ReturnUnit"
for CASE in direct-cycle unreachable-cycle; do
  pack "$CASE" "$T/generated/$CASE.ckir" 70
  observe 251 "$T/$CASE.rfn" "$CASE complete-graph rejection"
done
pack runtime-trap "$T/generated/runtime-trap.ckir" 4
observe 251 "$T/runtime-trap.rfn" "dynamic checked-add trap"
pack depth-boundary "$T/generated/depth-boundary.ckir" 70
observe 0 "$T/depth-boundary.rfn" "64-active-call evidence boundary"
pack depth-exhausted "$T/generated/depth-exhausted.ckir" 70
observe 252 "$T/depth-exhausted.rfn" "65th active-call evidence exhaustion"

# The outer component identity is versioned independently of the shared CKIR
# magic and offsets.
python3 "$OLD_PACKER" "$T/omgcomp" "$T/witness" "$T/canonical.ckir" "$T/elf" \
  --result 70 > "$T/old-frame.rfn"
observe 251 "$T/old-frame.rfn" "OMGRFN2 frame cross-rejection"

ELAPSED=$(($(date +%s) - STARTED))
echo "OMGRFN3 layer 5a: complete CKIR2 structure, exact-root calls/results, full-graph acyclicity, bounded call evaluation, 0/251/252 controls, and OMGRFN2 cross-rejection passed below Delta (${ELAPSED}s; ${PROCEDURES}/128 procedures)"
