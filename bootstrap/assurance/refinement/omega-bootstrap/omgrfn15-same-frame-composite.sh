#!/usr/bin/env sh
# OMGRFN15 immutable-frame R1--R5 composition for direct full-u32 subtraction.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN15 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN15 same-frame composite: skipped ($TOOL absent)"
    exit 0
  }
done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN15_COMPOSITE_TEMP:-0}" = 1 ]; then
  echo "OMGRFN15 same-frame composite: retained $T" >&2
else
  trap 'rm -rf "$T"' EXIT
fi

observe() {
  LABEL=$1 EXPECTED=$2 FRAME=$3 EXECUTABLE=$4
  set +e
  "$EXECUTABLE" < "$FRAME" > "$T/$LABEL.out" 2> "$T/$LABEL.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$LABEL.out" ] || {
    echo "OMGRFN15 same-frame composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/$LABEL.err" >&2
    exit 1
  }
}
run_pair() {
  observe "$4-native" "$3" "$2" "$T/$1.native"
  observe "$4-self" "$3" "$2" "$T/$1.self"
}

PYTHONPATH=$R python3 -B "$R/omgrfn15-materialize-r1-r2.py" "$T/checkers"
PYTHONPATH=$R python3 -B "$R/omgrfn15-materialize-r3-r5.py" "$T/checkers"
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"
"$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1

CHECKERS='r1 r2 r3 r4-lowering r4-source-result r5-structure r5-result r5-elf'
for NAME in $CHECKERS; do
  "$T/bc0" < "$T/checkers/$NAME.beta" > "$T/$NAME.native.asm"
  "$T/bc1" < "$T/checkers/$NAME.beta" > "$T/$NAME.self.asm"
  cmp "$T/$NAME.native.asm" "$T/$NAME.self.asm" >/dev/null
  "$ASM" < "$T/$NAME.native.asm" > "$T/$NAME.tape"
  BYTES=$(wc -c < "$T/$NAME.tape" | tr -d ' ')
  [ "$BYTES" -le 262140 ] || {
    echo "OMGRFN15 same-frame composite: $NAME tape $BYTES exceeds ceiling" >&2
    exit 1
  }
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.native" >/dev/null 2>&1
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.self" >/dev/null 2>&1
done

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir4.alp" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-checked-ir-v5-to-elf.alp" "$T/backend" >/dev/null

PYTHONPATH="$G:$R" python3 -B - "$T/resolver" "$T/lowerer" "$T/backend" "$T" <<'PY'
from pathlib import Path
import importlib.util
import subprocess
import sys

gate = Path("bootstrap/omega-bootstrap/gates/delta-resolved-to-ckir13-fixture.py")
spec = importlib.util.spec_from_file_location("omgrfn15_producer", gate)
producer = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(producer)
import omgrfn15_checker_model as model

resolver, lowerer, backend, output = map(Path, sys.argv[1:])
for profile, (name, path) in enumerate((
    ("success", producer.SUCCESS), ("underflow", producer.UNDERFLOW),
), 1):
    source = path.read_text(encoding="ascii")
    comp = producer.encode_source(source)
    witness = subprocess.run([str(resolver)], input=comp, stdout=subprocess.PIPE,
                             check=True).stdout
    ckir = subprocess.run([str(lowerer)],
                          input=producer.pack_lowering(comp, witness),
                          stdout=subprocess.PIPE, check=True).stdout
    elf = subprocess.run([str(backend)], input=ckir, stdout=subprocess.PIPE,
                         check=True).stdout
    assert witness == model.WITNESS[profile], f"{name} witness model drift"
    assert ckir == model.CKIR[profile], f"{name} CKIR model drift"
    assert elf == model.ELF[profile], f"{name} byte-complete ELF model drift"
    module = model.IR.decode(ckir)
    if profile == 1:
        assert model.IR.interpret(module) == 70
    else:
        try:
            model.IR.interpret(module)
        except model.IR.Ckir13Error:
            pass
        else:
            raise AssertionError("underflow CKIR did not trap")
    (output / f"{name}.omgc").write_bytes(comp)
    (output / f"{name}.witness").write_bytes(witness)
    (output / f"{name}.ckir").write_bytes(ckir)
    (output / f"{name}.elf").write_bytes(elf)
PY

for PROFILE in success underflow; do
  python3 -B "$R/omgrfn15_bundle.py" \
    "$T/$PROFILE.omgc" "$T/$PROFILE.witness" "$T/$PROFILE.ckir" \
    "$T/$PROFILE.elf" --result 70 > "$T/$PROFILE.rfn"
done

for NAME in $CHECKERS; do
  run_pair "$NAME" "$T/success.rfn" 0 "$NAME-success-positive"
done
for NAME in r1 r2 r3 r4-lowering r5-structure r5-elf; do
  run_pair "$NAME" "$T/underflow.rfn" 0 "$NAME-underflow-structural"
done
run_pair r4-source-result "$T/underflow.rfn" 251 r4-source-result-underflow-trap
run_pair r5-result "$T/underflow.rfn" 251 r5-result-underflow-trap

python3 -B "$R/omgrfn15_bundle.py" \
  "$T/success.omgc" "$T/underflow.witness" "$T/success.ckir" "$T/success.elf" \
  --result 70 > "$T/source-witness-cross.rfn"
python3 -B "$R/omgrfn15_bundle.py" \
  "$T/underflow.omgc" "$T/success.witness" "$T/underflow.ckir" "$T/underflow.elf" \
  --result 70 > "$T/witness-ckir-cross.rfn"
python3 -B "$R/omgrfn15_bundle.py" \
  "$T/success.omgc" "$T/success.witness" "$T/underflow.ckir" "$T/underflow.elf" \
  --result 70 > "$T/source-ckir-cross.rfn"
python3 -B "$R/omgrfn15_bundle.py" \
  "$T/success.omgc" "$T/success.witness" "$T/success.ckir" "$T/underflow.elf" \
  --result 70 > "$T/ckir-elf-cross.rfn"

PYTHONPATH=$R python3 -B - "$T/success.rfn" "$T" <<'PY'
from pathlib import Path
import struct
import sys
import omgrfn15_checker_model as model

raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
omg, witness, ckir, elf = struct.unpack_from("<4I", raw, 16)
witness_at = 40 + omg
ckir_at = witness_at + witness
elf_at = ckir_at + ckir

def put(name, data):
    (out / f"{name}.rfn").write_bytes(data)

x = bytearray(raw); x[6] = ord("E"); struct.pack_into("<I", x, 8, 14); put("outer14", x)
x = bytearray(raw); struct.pack_into("<II", x, 32, 71, 71); put("claim71", x)
x = bytearray(raw); source = bytes(x[40:40 + omg]); marker = model.source_model()[1]
at = source.find(marker); assert at >= 0
literal = source.find(b"4294967290", at, at + len(marker)); assert literal >= 0
x[40 + literal + 9] = ord("1"); put("source-literal", x)
x = bytearray(raw); x[witness_at + 6] = ord("4"); put("witness4", x)
x = bytearray(raw); struct.pack_into("<H", x, ckir_at + 8, 12); put("ckir12", x)

counts = dict(zip(model.IR.COUNT_NAMES, struct.unpack_from("<21I", raw, ckir_at + 16)[2:]))
cursor = ckir_at + model.IR.HEADER.size
offsets = {}
for table in model.IR.TABLE_ORDER:
    offsets[table] = cursor
    cursor += counts[table] * model.IR.ROWS[table].size
x = bytearray(raw); x[offsets["operations"] + 16 * model.IR.ROWS["operations"].size + 12] = 8
put("ckir-opcode", x)
x = bytearray(raw); needle = b"\x2b\x85"; at = bytes(x).find(needle, elf_at)
assert at >= elf_at; x[at] = 3; put("elf-add", x)
put("trailing", raw + b"\0")
put("resource", raw + b"\0" * (4_497_545 - len(raw)))
PY

for NAME in $CHECKERS; do
  run_pair "$NAME" "$T/outer14.rfn" 251 "$NAME-outer14"
  run_pair "$NAME" "$T/trailing.rfn" 251 "$NAME-trailing"
  run_pair "$NAME" "$T/resource.rfn" 252 "$NAME-resource"
done
run_pair r2 "$T/source-literal.rfn" 251 r2-source-literal
run_pair r2 "$T/witness4.rfn" 251 r2-witness4
run_pair r2 "$T/source-witness-cross.rfn" 251 r2-source-witness-cross
run_pair r3 "$T/witness4.rfn" 251 r3-witness4
run_pair r3 "$T/ckir12.rfn" 251 r3-ckir12
run_pair r3 "$T/witness-ckir-cross.rfn" 251 r3-witness-ckir-cross
run_pair r4-lowering "$T/source-literal.rfn" 251 r4-source-literal
run_pair r4-lowering "$T/source-ckir-cross.rfn" 251 r4-source-ckir-cross
run_pair r4-source-result "$T/source-literal.rfn" 251 r4-source-result-literal
run_pair r5-structure "$T/ckir-opcode.rfn" 251 r5-opcode
run_pair r5-result "$T/claim71.rfn" 251 r5-result-claim
run_pair r5-elf "$T/elf-add.rfn" 251 r5-elf-add
run_pair r5-elf "$T/ckir-elf-cross.rfn" 251 r5-ckir-elf-cross

for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-elf; do
  run_pair "$NAME" "$T/claim71.rfn" 0 "$NAME-claim-opacity"
done

echo "OMGRFN15 same-frame composite: R1-R5 native/self direct full-u32 subtraction, exact OMGRSW5/CKIR13/ELF, trapping underflow phase ownership, cross-pairs, and mutation teeth passed"
