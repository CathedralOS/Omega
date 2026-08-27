#!/usr/bin/env sh
# OMGRFN8 immutable-frame R1--R5 composition for bool-only logical negation.
set -eu

STARTED=$(date +%s)
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN8 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN8 same-frame composite: skipped ($TOOL absent)"
    exit 0
  }
done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
PRIMARY=$G/fixtures/ckir6-logical-not/general.omg
BUILDER=$G/delta-resolved-to-ckir5-fixture.py
RESOLVER=$C/omega-bootstrap-resolve.alp
LOWERER=$C/omega-bootstrap-resolved-to-ckir4.alp
BACKEND=$C/omega-bootstrap-checked-ir-v5-to-elf.alp
PACKER=$R/omgrfn8_bundle.py
MATERIALIZE12=$R/omgrfn8-materialize-r1-r2.py
MATERIALIZE35=$R/omgrfn8-materialize-r3-r5.py
for REQUIRED in "$PRIMARY" "$BUILDER" "$RESOLVER" "$LOWERER" "$BACKEND" \
  "$PACKER" "$MATERIALIZE12" "$MATERIALIZE35"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN8 same-frame composite: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN8_COMPOSITE_TEMP:-0}" = 1 ]; then
  echo "OMGRFN8 same-frame composite: retained $T" >&2
else
  trap 'rm -rf "$T"' EXIT
fi
: > "$T/timings.tsv"

observe() { # label expected input output executable
  LABEL=$1 EXPECTED=$2 INPUT=$3 OUTPUT=$4 EXECUTABLE=$5
  BEGIN=$(date +%s)
  set +e
  "$EXECUTABLE" < "$INPUT" > "$OUTPUT" 2> "$OUTPUT.stderr"
  ACTUAL=$?
  set -e
  FINISH=$(date +%s)
  printf '%s\t%s\n' "$((FINISH-BEGIN))" "$LABEL" >> "$T/timings.tsv"
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN8 same-frame composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,16p' "$OUTPUT.stderr" >&2
    exit 1
  }
  [ ! -s "$OUTPUT" ] || {
    echo "OMGRFN8 same-frame composite: $LABEL published bytes" >&2
    exit 1
  }
}

python3 -B "$MATERIALIZE12" "$T/checkers"
python3 -B "$MATERIALIZE35" "$T/checkers"

# Build the Beta compiler's self image once. Each responsibility is then
# translated independently by both compiler generations; one matching tape is
# stamped into distinct native/self executables.
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
  TAPE_BYTES=$(wc -c < "$T/$NAME.tape" | tr -d ' ')
  [ "$TAPE_BYTES" -le 262140 ] || {
    echo "OMGRFN8 same-frame composite: $NAME tape $TAPE_BYTES exceeds ceiling" >&2
    exit 1
  }
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.native" >/dev/null 2>&1
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.self" >/dev/null 2>&1
done

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend" >/dev/null

# Build one full OMGRSW3 carrier plus compact SW1/SW2 least-resolution controls.
python3 -B - "$PRIMARY" "$BUILDER" "$T" <<'PY'
from pathlib import Path
import importlib.util, sys

primary, helper, output = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
spec = importlib.util.spec_from_file_location("omgrfn8_fixture", helper)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)

sources = {
    "primary": (primary.read_text(encoding="ascii"), "SumProducer", "run"),
    "sw1": ("""data BoolOne {}
machine BoolOne::run(&mut self) -> u8 {
    transition !!false { true -> failed() false -> passed() }
    state failed(&mut self) { 0 }
    state passed(&mut self) { 70 }
}
""", "BoolOne", "run"),
    "sw2": ("""data BoolGate {}
data BoolHost { gate: BoolGate; }
machine BoolGate::check(&self) -> u8 {
    transition !!false { true -> failed() false -> passed() }
    state failed(&self) { 0 }
    state passed(&self) { 70 }
}
machine BoolHost::run(&mut self) -> u8 { self.gate.check() }
""", "BoolHost", "run"),
}
for name, (source, owner, machine) in sources.items():
    (output / f"{name}.omg").write_text(source, encoding="ascii")
    (output / f"{name}.omgc").write_bytes(module.encode_source(source, owner, machine))
PY

for NAME in primary sw1 sw2; do
  "$T/resolver" < "$T/$NAME.omgc" > "$T/$NAME.witness"
  python3 -B - "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.omglow" "$BUILDER" <<'PY'
from pathlib import Path
import importlib.util, struct, sys
comp, witness, output, helper = map(Path, sys.argv[1:])
spec = importlib.util.spec_from_file_location("omgrfn8_fixture", helper)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)
raw = witness.read_bytes()
major = struct.unpack_from("<H", raw, 8)[0]
output.write_bytes(module.pack_lowering(comp.read_bytes(), raw, version=7, resolution=major))
PY
  "$T/lowerer" < "$T/$NAME.omglow" > "$T/$NAME.ckir6"
  "$T/backend" < "$T/$NAME.ckir6" > "$T/$NAME.elf"
  python3 -B "$PACKER" "$T/$NAME.omgc" "$T/$NAME.witness" \
    "$T/$NAME.ckir6" "$T/$NAME.elf" --result 70 > "$T/$NAME.rfn"
done

python3 -B "$PACKER" "$T/primary.omgc" "$T/sw1.witness" \
  "$T/primary.ckir6" "$T/primary.elf" --result 70 > "$T/source-witness-cross.rfn"
python3 -B "$PACKER" "$T/primary.omgc" "$T/primary.witness" \
  "$T/sw1.ckir6" "$T/sw1.elf" --result 70 > "$T/witness-ckir-cross.rfn"
python3 -B "$PACKER" "$T/primary.omgc" "$T/primary.witness" \
  "$T/primary.ckir6" "$T/sw1.elf" --result 70 > "$T/ckir-elf-cross.rfn"

python3 -B - "$T/primary.rfn" "$T" <<'PY'
from pathlib import Path
import struct, sys

raw = Path(sys.argv[1]).read_bytes()
output = Path(sys.argv[2])
oc, ow, ck, el = struct.unpack_from("<4I", raw, 16)
witness = 40 + oc
ckir = witness + ow
elf = ckir + ck

def put(name, contents):
    (output / f"{name}.rfn").write_bytes(contents)

x = bytearray(raw)
x[6] = ord("7")
struct.pack_into("<I", x, 8, 7)
put("outer7", x)

x = bytearray(raw)
struct.pack_into("<II", x, 32, 71, 71)
put("claim71", x)

x = bytearray(raw)
struct.pack_into("<II", x, 32, 326, 70)
put("claim326", x)

x = bytearray(raw)
at = bytes(x).find(b"!!false", 40, witness)
if at < 0:
    raise SystemExit("missing source logical-not anchor")
x[at:at + 2] = b"  "
put("source-no-bang", x)

x = bytearray(raw)
x[witness + 20] ^= 1
put("witness-count", x)

counts = struct.unpack_from("<19I", raw, ckir + 24)
offset = ckir + 100
widths = (24, 20, 16, 20, 20, 16, 36, 20, 32, 20, 24, 4, 40, 4, 52, 24, 12)
tables = []
for count, width in zip(counts[:17], widths):
    tables.append(offset)
    offset += count * width
logical = next(i for i in range(counts[12]) if raw[tables[12] + i * 40 + 12] == 15)
x = bytearray(raw)
struct.pack_into("<I", x, tables[12] + logical * 40 + 32, 1)
put("logical-immediate", x)

x = bytearray(raw)
x[ckir + 8] = 5
put("ckir5-major", x)

x = bytearray(raw)
pattern = b"\x83\xf0\x01"
at = bytes(x).find(pattern, elf, elf + el)
if at < 0:
    raise SystemExit("missing logical-not ELF template")
x[at + 2] = 2
put("elf-xor", x)
put("trailing", raw + b"\0")
PY

run_pair() { # checker frame status label
  NAME=$1 FRAME=$2 EXPECTED=$3 LABEL=$4
  observe "$LABEL-native" "$EXPECTED" "$FRAME" "$T/$LABEL.native.out" "$T/$NAME.native"
  observe "$LABEL-self" "$EXPECTED" "$FRAME" "$T/$LABEL.self.out" "$T/$NAME.self"
}

for NAME in $CHECKERS; do run_pair "$NAME" "$T/primary.rfn" 0 "$NAME-positive"; done
for CONTROL in sw1 sw2; do
  run_pair r1 "$T/$CONTROL.rfn" 0 "r1-$CONTROL"
  run_pair r2 "$T/$CONTROL.rfn" 0 "r2-$CONTROL"
done

for NAME in $CHECKERS; do run_pair "$NAME" "$T/outer7.rfn" 251 "$NAME-outer7"; done
for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-elf; do
  run_pair "$NAME" "$T/claim71.rfn" 0 "$NAME-claim-opacity"
done
run_pair r5-result "$T/claim71.rfn" 251 r5-result-claim
for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-elf; do
  run_pair "$NAME" "$T/claim326.rfn" 0 "$NAME-full-result-opacity"
done
run_pair r5-result "$T/claim326.rfn" 251 r5-result-full-result-not-projection

run_pair r1 "$T/source-witness-cross.rfn" 0 r1-source-witness-cross-opacity
run_pair r2 "$T/source-witness-cross.rfn" 251 r2-source-witness-cross
run_pair r3 "$T/source-witness-cross.rfn" 251 r3-source-witness-cross
run_pair r4-lowering "$T/source-witness-cross.rfn" 251 r4-lowering-source-witness-cross
for NAME in r4-source-result r5-structure r5-result r5-elf; do run_pair "$NAME" "$T/source-witness-cross.rfn" 0 "$NAME-source-witness-opacity"; done

for NAME in r1 r2; do run_pair "$NAME" "$T/witness-ckir-cross.rfn" 0 "$NAME-witness-ckir-opacity"; done
run_pair r3 "$T/witness-ckir-cross.rfn" 251 r3-witness-ckir-cross
run_pair r4-lowering "$T/witness-ckir-cross.rfn" 251 r4-lowering-witness-ckir-cross
run_pair r4-source-result "$T/witness-ckir-cross.rfn" 0 r4-result-ckir-opacity
for NAME in r5-structure r5-result r5-elf; do run_pair "$NAME" "$T/witness-ckir-cross.rfn" 0 "$NAME-valid-secondary-ckir"; done

for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-result; do
  run_pair "$NAME" "$T/ckir-elf-cross.rfn" 0 "$NAME-ckir-elf-opacity"
done
run_pair r5-elf "$T/ckir-elf-cross.rfn" 251 r5-elf-valid-cross-pair

run_pair r1 "$T/source-no-bang.rfn" 0 r1-source-opacity
run_pair r2 "$T/source-no-bang.rfn" 251 r2-source-bang
run_pair r3 "$T/source-no-bang.rfn" 0 r3-source-opacity
run_pair r4-lowering "$T/source-no-bang.rfn" 251 r4-lowering-source-bang
run_pair r4-source-result "$T/source-no-bang.rfn" 251 r4-result-source-bang
for NAME in r5-structure r5-result r5-elf; do run_pair "$NAME" "$T/source-no-bang.rfn" 0 "$NAME-source-opacity"; done

run_pair r1 "$T/witness-count.rfn" 0 r1-witness-opacity
for NAME in r2 r3 r4-lowering; do run_pair "$NAME" "$T/witness-count.rfn" 251 "$NAME-witness"; done
for NAME in r4-source-result r5-structure r5-result r5-elf; do run_pair "$NAME" "$T/witness-count.rfn" 0 "$NAME-witness-opacity"; done

for NAME in r1 r2 r4-source-result; do run_pair "$NAME" "$T/logical-immediate.rfn" 0 "$NAME-ckir-opacity"; done
for NAME in r3 r5-structure r5-result r5-elf; do run_pair "$NAME" "$T/logical-immediate.rfn" 251 "$NAME-logical-immediate"; done
run_pair r4-lowering "$T/logical-immediate.rfn" 0 r4-lowering-intrinsic-opacity

for NAME in r1 r2 r4-lowering r4-source-result; do run_pair "$NAME" "$T/elf-xor.rfn" 0 "$NAME-elf-opacity"; done
for NAME in r3 r5-structure r5-result; do run_pair "$NAME" "$T/elf-xor.rfn" 0 "$NAME-elf-opacity"; done
run_pair r5-elf "$T/elf-xor.rfn" 251 r5-elf-xor

for NAME in r3 r4-lowering r5-structure r5-result r5-elf; do run_pair "$NAME" "$T/ckir5-major.rfn" 251 "$NAME-ckir5-cross"; done
for NAME in $CHECKERS; do run_pair "$NAME" "$T/trailing.rfn" 251 "$NAME-trailing"; done

ELAPSED=$(( $(date +%s) - STARTED ))
echo "OMGRFN8 same-frame composite: R1-R5 native/self, least SW1/2 controls, full SW3 result70, ownership mutations, version teeth, and exact ELF passed"
echo "OMGRFN8 same-frame composite: elapsed=${ELAPSED}s; checkers materialized and compiled once per Beta generation"
