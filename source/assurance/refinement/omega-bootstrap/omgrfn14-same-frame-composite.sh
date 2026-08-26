#!/usr/bin/env sh
# OMGRFN14 immutable-frame R1--R5 composition for producer-backed CKIR12 views.
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
  *) echo "OMGRFN14 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN14 same-frame composite: skipped ($TOOL absent)"
    exit 0
  }
done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN14_COMPOSITE_TEMP:-0}" = 1 ]; then
  echo "OMGRFN14 same-frame composite: retained $T" >&2
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
    echo "OMGRFN14 same-frame composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/$LABEL.err" >&2
    exit 1
  }
}
run_pair() {
  observe "$4-native" "$3" "$2" "$T/$1.native"
  observe "$4-self" "$3" "$2" "$T/$1.self"
}

PYTHONPATH=$R python3 -B "$R/omgrfn14-materialize-r1-r2.py" "$T/checkers"
PYTHONPATH=$R python3 -B "$R/omgrfn14-materialize-r3-r5.py" "$T/checkers"
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
    echo "OMGRFN14 same-frame composite: $NAME tape $BYTES exceeds ceiling" >&2
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

gate = Path("bootstrap/omega-bootstrap/gates/delta-resolved-to-ckir12-fixture.py")
spec = importlib.util.spec_from_file_location("omgrfn14_producer", gate)
producer = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(producer)
import omgrfn14_checker_model as model

resolver, lowerer, backend, output = map(Path, sys.argv[1:])
for profile, (name, path) in enumerate((
    ("one", producer.ONE_BYTE_SOURCE), ("empty", producer.EMPTY_SOURCE),
), 1):
    frame, witness, ckir = producer.produce(
        resolver, lowerer, producer.source_text(path)
    )
    comp_len = int.from_bytes(frame[20:24], "little")
    comp = frame[32:32 + comp_len]
    assert ckir == model.CKIR[profile], f"{name} producer/reference CKIR drift"
    elf = subprocess.run([str(backend)], input=ckir, stdout=subprocess.PIPE,
                         check=True).stdout
    assert elf == model.ELF[profile], f"{name} backend/template drift"
    (output / f"{name}.omgc").write_bytes(comp)
    (output / f"{name}.witness").write_bytes(witness)
    (output / f"{name}.ckir").write_bytes(ckir)
    (output / f"{name}.elf").write_bytes(elf)
PY

for PROFILE in one empty; do
  python3 -B "$R/omgrfn14_bundle.py" \
    "$T/$PROFILE.omgc" "$T/$PROFILE.witness" "$T/$PROFILE.ckir" \
    "$T/$PROFILE.elf" --result 70 > "$T/$PROFILE.rfn"
  for NAME in $CHECKERS; do
    run_pair "$NAME" "$T/$PROFILE.rfn" 0 "$NAME-$PROFILE-positive"
  done
done

python3 -B "$R/omgrfn14_bundle.py" \
  "$T/one.omgc" "$T/one.witness" "$T/empty.ckir" "$T/empty.elf" \
  --result 70 > "$T/cross.rfn"

PYTHONPATH=$R python3 -B - "$T/one.rfn" "$T" <<'PY'
from pathlib import Path
import struct
import sys
import omgrfn14_checker_model as model

raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
omg, witness, ckir, elf = struct.unpack_from("<4I", raw, 16)
witness_at = 40 + omg
ckir_at = witness_at + witness
elf_at = ckir_at + ckir

def put(name, data):
    (out / f"{name}.rfn").write_bytes(data)

x = bytearray(raw); x[6] = ord("D"); struct.pack_into("<I", x, 8, 13); put("outer13", x)
x = bytearray(raw); struct.pack_into("<II", x, 32, 71, 71); put("claim71", x)
x = bytearray(raw); source = bytes(x[40:40 + omg]); marker = model.source_model()[1]
at = source.find(marker); assert at >= 0; literal = source.find(b'"F"', at, at + len(marker)); assert literal >= 0
x[40 + literal + 1] = ord("G"); put("source-literal", x)
type_at = (witness_at + 84 + struct.unpack_from("<I", raw, witness_at + 20)[0] * 36
           + struct.unpack_from("<I", raw, witness_at + 24)[0] * 48
           + struct.unpack_from("<I", raw, witness_at + 28)[0] * 28
           + struct.unpack_from("<I", raw, witness_at + 32)[0] * 28)
x = bytearray(raw); x[type_at + 4 * 24 + 4] = 5; put("witness-kind", x)
x = bytearray(raw); x[ckir_at + 8] = 11; put("ckir11", x)

def table_at(name):
    counts = dict(zip(model.FIXTURE.ir12.COUNT_NAMES,
                      struct.unpack_from("<21I", raw, ckir_at + 16)[2:]))
    cursor = ckir_at + model.FIXTURE.ir12.HEADER.size
    for table in model.FIXTURE.ir12.TABLE_ORDER:
        if table == name: return cursor
        cursor += counts[table] * model.FIXTURE.ir12.ROWS[table].size
    raise AssertionError(name)

x = bytearray(raw); x[table_at("constants") + 16] = 71; put("ckir-literal", x)
x = bytearray(raw); x[table_at("operations") + 12] = 23; put("ckir-opcode", x)
x = bytearray(raw); x[table_at("blocks") + 4 * model.FIXTURE.ir12.ROWS["blocks"].size + 9] = 0; put("ckir-synthetic", x)
x = bytearray(raw); x[elf_at + 4096 + 62] = 2; put("elf-length", x)
x = bytearray(raw); x[elf_at + 8192] = 71; put("elf-literal", x)
put("trailing", raw + b"\0")
put("resource", raw + b"\0" * (4_497_545 - len(raw)))
PY

for NAME in $CHECKERS; do
  run_pair "$NAME" "$T/outer13.rfn" 251 "$NAME-outer13"
  run_pair "$NAME" "$T/trailing.rfn" 251 "$NAME-trailing"
  run_pair "$NAME" "$T/resource.rfn" 252 "$NAME-resource"
done
run_pair r2 "$T/source-literal.rfn" 251 r2-source-literal
run_pair r2 "$T/witness-kind.rfn" 251 r2-witness-kind
run_pair r3 "$T/witness-kind.rfn" 251 r3-witness-kind
run_pair r3 "$T/ckir-literal.rfn" 251 r3-ckir-literal
run_pair r4-lowering "$T/source-literal.rfn" 251 r4-source-literal
run_pair r4-lowering "$T/cross.rfn" 251 r4-source-ckir-cross
run_pair r4-source-result "$T/source-literal.rfn" 251 r4-source-result-literal
run_pair r5-structure "$T/ckir-opcode.rfn" 251 r5-opcode
run_pair r5-structure "$T/ckir-synthetic.rfn" 251 r5-synthetic
run_pair r5-result "$T/claim71.rfn" 251 r5-result-claim
run_pair r5-elf "$T/elf-length.rfn" 251 r5-elf-length
run_pair r5-elf "$T/elf-literal.rfn" 251 r5-elf-literal

for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-elf; do
  run_pair "$NAME" "$T/claim71.rfn" 0 "$NAME-claim-opacity"
done

echo "OMGRFN14 same-frame composite: R1-R5 native/self producer-backed one-byte true edge and empty false bypass, exact CKIR12/ELF, profile crossing, ownership, and mutation teeth passed"
