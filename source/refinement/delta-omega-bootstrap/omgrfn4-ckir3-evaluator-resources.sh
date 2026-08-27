#!/usr/bin/env sh
# Focused OMGRFN4 responsibility-5 CKIR-only evaluator boundaries.
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
  *) echo "OMGRFN4 CKIR3 evaluator resources: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN4 CKIR3 evaluator resources: skipped ($TOOL absent)"
    exit 0
  }
done

ENVELOPE="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4-component-envelope.beta"
CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4-ckir3-evaluator-resources.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4_bundle.py"
IR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v3_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v3_reference.py"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v3-to-elf.alp"
for REQUIRED in "$ENVELOPE" "$CHECKER" "$PACKER" "$IR_REFERENCE" "$ELF_REFERENCE" "$BACKEND"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN4 CKIR3 evaluator resources: missing $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_OMGRFN4_EVAL_TEMP:-0}" = 1 ]; then
    echo "OMGRFN4 CKIR3 evaluator resources: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT
: > "$T/timings.tsv"

python3 - "$T/observe.py" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_text(r'''#!/usr/bin/env python3
from pathlib import Path
import os, signal, subprocess, sys, time
label, expected, timeout, source, output, timings, *command = sys.argv[1:]
started = time.monotonic()
with open(source, "rb") as input_file:
    process = subprocess.Popen(command, stdin=input_file, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=float(timeout))
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        Path(output).write_bytes(stdout)
        Path(output + ".stderr").write_bytes(stderr)
        raise SystemExit(f"{label} exceeded {timeout}s")
elapsed = time.monotonic() - started
Path(output).write_bytes(stdout)
Path(output + ".stderr").write_bytes(stderr)
with open(timings, "a", encoding="ascii") as out:
    out.write(f"{elapsed:.6f}\t{label}\n")
if process.returncode != int(expected):
    if stderr:
        sys.stderr.buffer.write(stderr[-4096:])
    raise SystemExit(f"{label} returned {process.returncode}, expected {expected}")
''', encoding="ascii")
PY

observe() { # label expected timeout stdin stdout command...
  OBS_LABEL=$1 OBS_EXPECTED=$2 OBS_TIMEOUT=$3 OBS_INPUT=$4 OBS_OUTPUT=$5
  shift 5
  python3 "$T/observe.py" "$OBS_LABEL" "$OBS_EXPECTED" "$OBS_TIMEOUT" \
    "$OBS_INPUT" "$OBS_OUTPUT" "$T/timings.tsv" "$@"
}

stamp_beta_compiler "$T/bc" >/dev/null
cat "$ENVELOPE" "$CHECKER" > "$T/check.beta"
PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/check.beta")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN4 CKIR3 evaluator resources: $PROCEDURES procedures exceeds 128" >&2
  exit 1
}
MAX_LOCALS=$(python3 - "$T/check.beta" <<'PY'
import re, sys
source = open(sys.argv[1], encoding="ascii").read()
maximum = 0
for match in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{", source, re.M):
    end = source.find("\nproc ", match.end())
    body = source[match.end():end if end >= 0 else len(source)]
    params = sum(bool(item.strip()) for item in match.group(1).split(","))
    maximum = max(maximum, params + len(re.findall(r"\blet\s+[A-Za-z_]\w*", body)))
print(maximum)
PY
)
[ "$MAX_LOCALS" -le 32 ] || {
  echo "OMGRFN4 CKIR3 evaluator resources: $MAX_LOCALS locals exceeds 32" >&2
  exit 1
}

observe build-beta 0 90 "$T/check.beta" "$T/check.asm" "$T/bc"
observe build-alpha 0 90 "$T/check.asm" "$T/check.tape" \
  "$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
TAPE_BYTES=$(wc -c < "$T/check.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || {
  echo "OMGRFN4 CKIR3 evaluator resources: $TAPE_BYTES tape bytes exceeds 262140" >&2
  exit 1
}
stamp_seed "$T/check.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$T/check" >/dev/null 2>&1

mkdir "$T/cases"
python3 - "$T/cases" <<'PY'
from pathlib import Path
import struct, sys

out = Path(sys.argv[1])
sys.path.insert(0, "source/on-ramp/omega-bootstrap/gates")
import checked_ir_v3_reference as reference

NO = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH16I")
ROWS = (
    struct.Struct("<IBBHIIII"), struct.Struct("<IIIIB3x"),
    struct.Struct("<IIII"), struct.Struct("<IIBBHIIIIII"),
    struct.Struct("<IIIII"), struct.Struct("<IIBBHIIIII"),
    struct.Struct("<IIIII"), struct.Struct("<IIIIII"),
    struct.Struct("<I"), struct.Struct("<IIIBBHIIIIII"),
    struct.Struct("<I"), struct.Struct("<IIIBBHIIIIIII"),
)

def encode(name, tables, values, places):
    payload = b"".join(row_type.pack(*row) for table, row_type in zip(tables, ROWS) for row in table)
    counts = tuple(len(table) for table in tables)
    header_counts = (*counts[:7], counts[9], counts[10], counts[11],
                     values, places, counts[7], counts[8])
    raw = HEADER.pack(b"OMGCKIR\0", 3, 0, 1, 1, 0,
                      HEADER.size + len(payload), *header_counts) + payload
    (out / (name + ".ckir3")).write_bytes(raw)
    return raw

def chain(name, count, cycle=False):
    types = [(0,4,0,0,0,0,0,0),(1,3,0,0,0,0,0,1),
             (2,2,0,0,0,0,0,0x7FFF_FFFF),(3,1,0,0,0,0,0,255)]
    records = [(0,0,0,0,0)]
    machines=[]; blocks=[]; operations=[]; operands=[]; terms=[]
    for machine in range(count):
        op_start=len(operations)
        if machine+1<count:
            operations.append((len(operations),machine,machine,2,2,0,machine,0,len(operands),0,0,0))
            target=0 if cycle and machine==0 else machine+1
            operations.append((len(operations),machine,machine,10,1,0,machine,3,len(operands),1,target,0))
            operands.append(machine)
        else:
            operations.append((len(operations),machine,machine,1,1,0,machine,3,len(operands),0,70,0))
        machines.append((machine,0,2,0,0,3,0,0,machine,1,machine))
        blocks.append((machine,machine,2,0,0,0,0,op_start,len(operations)-op_start,machine))
        terms.append((machine,machine,machine,4,0,0,machine,NO,len(operands),0,NO,len(operands),0))
    terms=[row[:8]+(len(operands),row[9],row[10],len(operands),row[12]) for row in terms]
    tables=(types,records,[],machines,[],blocks,[],[],[],operations,
            [(value,) for value in operands],terms)
    return encode(name,tables,count,count-1)

def loop(name, limit):
    types=[(0,4,0,0,0,0,0,0),(1,2,0,0,0,0,0,0x7FFF_FFFF),
           (2,3,0,0,0,0,0,1),(3,1,0,0,0,0,0,255)]
    records=[(0,0,0,0,0)]
    machines=[(0,0,2,0,0,3,0,0,0,3,0)]
    blocks=[(0,0,2,0,0,0,0,0,1,0),(1,0,2,0,0,0,1,1,4,1),(2,0,2,0,0,1,0,5,1,2)]
    block_params=[(0,1,0,1,0)]
    operations=[
        (0,0,0,1,1,0,1,1,0,0,0,0),
        (1,0,1,1,1,0,2,1,0,0,limit,0),
        (2,0,1,1,1,0,3,1,0,0,1,0),
        (3,0,1,9,1,0,4,2,0,2,0,0),
        (4,0,1,8,1,0,5,1,2,2,0,0),
        (5,0,2,1,1,0,6,3,4,0,70,0),
    ]
    operands=[0,2,0,3,1,5]
    terms=[
        (0,0,0,1,0,0,NO,1,4,1,NO,5,0),
        (1,0,1,2,0,0,4,1,5,1,2,6,0),
        (2,0,2,4,0,0,6,NO,6,0,NO,6,0),
    ]
    tables=(types,records,[],machines,[],blocks,block_params,[],[],operations,
            [(value,) for value in operands],terms)
    return encode(name,tables,7,0)

cases = [
    ("frames-64", chain("frames-64",64), 0, "70"),
    ("frames-65", chain("frames-65",65), 252, "active machine-frame exhaustion"),
    ("entries-65536", loop("entries-65536",65533), 0, "70"),
    ("entries-65537", loop("entries-65537",65534), 252, "dynamic block-entry exhaustion"),
]
cycle=chain("call-cycle",64,True)
try:
    reference.decode(cycle)
except Exception as error:
    if "cyclic machine calls" not in str(error):
        raise
else:
    raise SystemExit("independent reference accepted call cycle")

manifest=[]
for name, raw, expected, oracle in cases:
    module=reference.decode(raw)
    try:
        result=reference.interpret(module,step_limit=65536,frame_limit=64)
    except Exception as error:
        if expected != 252 or oracle not in str(error):
            raise
    else:
        if expected != 0 or result != 70:
            raise SystemExit(f"{name}: independent result drift")
    manifest.append(f"{name}\t{expected}\t{len(raw)}\t{oracle}")
(out / "manifest.tsv").write_text("\n".join(manifest)+"\n",encoding="ascii")
print("independent CKIR3 resource oracle: 64/65 frames and 65536/65537 entries passed")
PY

printf opaque-omgcomp > "$T/omgcomp"
printf opaque-witness > "$T/witness"
printf opaque-elf > "$T/opaque.elf"

observe build-cargo 0 120 /dev/null "$T/cargo.stdout" \
  cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
observe build-backend 0 90 /dev/null "$T/backend.stdout" \
  env DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend"

TAB=$(printf '\t')
CASE_COUNT=0
while IFS="$TAB" read -r NAME EXPECTED BYTES ORACLE; do
  CASE_COUNT=$((CASE_COUNT+1))
  INPUT="$T/cases/$NAME.ckir3"
  [ "$(wc -c < "$INPUT" | tr -d ' ')" -eq "$BYTES" ] || exit 1
  observe "reference-$NAME" 0 30 /dev/null "$T/$NAME.reference" \
    python3 -B "$IR_REFERENCE" validate "$INPUT"
  observe "backend-$NAME" 0 90 "$INPUT" "$T/$NAME.elf" "$T/backend"
  [ -s "$T/$NAME.elf" ] || {
    echo "OMGRFN4 CKIR3 evaluator resources: $NAME backend emitted no ELF" >&2
    exit 1
  }
  observe "elf-$NAME" 0 90 /dev/null "$T/$NAME.elf-check" \
    python3 -B "$ELF_REFERENCE" check "$INPUT" "$T/$NAME.elf"
  observe "pack-$NAME" 0 30 /dev/null "$T/$NAME.rfn" \
    python3 "$PACKER" "$T/omgcomp" "$T/witness" "$INPUT" "$T/$NAME.elf" --result 70
  observe "checker-$NAME" "$EXPECTED" 30 "$T/$NAME.rfn" "$T/$NAME.stdout" "$T/check"
  [ ! -s "$T/$NAME.stdout" ] || {
    echo "OMGRFN4 CKIR3 evaluator resources: $NAME checker published output" >&2
    exit 1
  }
done < "$T/cases/manifest.tsv"
[ "$CASE_COUNT" -eq 4 ] || exit 1

# Cyclic CFG is the positive counter family; a cyclic machine-call graph is a
# distinct malformed control and never becomes evaluator exhaustion.
observe reference-call-cycle 1 30 /dev/null "$T/call-cycle.reference" \
  python3 -B "$IR_REFERENCE" validate "$T/cases/call-cycle.ckir3"
observe backend-call-cycle 251 30 "$T/cases/call-cycle.ckir3" "$T/call-cycle.backend" "$T/backend"
[ ! -s "$T/call-cycle.backend" ] || exit 1
observe pack-call-cycle 0 30 /dev/null "$T/call-cycle.rfn" \
  python3 "$PACKER" "$T/omgcomp" "$T/witness" "$T/cases/call-cycle.ckir3" "$T/opaque.elf" --result 70
observe checker-call-cycle 251 30 "$T/call-cycle.rfn" "$T/call-cycle.stdout" "$T/check"
[ ! -s "$T/call-cycle.stdout" ] || exit 1

python3 - "$T/timings.tsv" <<'PY'
from collections import defaultdict
from pathlib import Path
import sys
rows=[]; phases=defaultdict(float)
for line in Path(sys.argv[1]).read_text().splitlines():
    elapsed,label=line.split("\t",1); seconds=float(elapsed)
    rows.append((seconds,label)); phases[label.split("-",1)[0]]+=seconds
print("OMGRFN4 CKIR3 evaluator resource timings: " +
      " ".join(f"{key}={phases[key]:.2f}s" for key in sorted(phases)) +
      f"; command-total={sum(x for x,_ in rows):.2f}s; slowest={max(rows)[1]} {max(rows)[0]:.2f}s")
PY
echo "OMGRFN4 CKIR3 evaluator resources: 64/65 active frames, global 65536/65537 block entries, cyclic-CFG/call-cycle separation, exact result/252-empty, and valid over-boundary ELF passed (${PROCEDURES}/128 procedures; ${MAX_LOCALS}/32 locals; ${TAPE_BYTES}/262140 tape bytes)"
