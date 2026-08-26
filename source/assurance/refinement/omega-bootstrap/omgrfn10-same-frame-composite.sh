#!/usr/bin/env sh
# OMGRFN10 immutable-frame R1--R5 composition for scalar equality.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in "Darwin arm64") ;; *) echo "OMGRFN10 same-frame composite: skipped (requires Darwin arm64)"; exit 0;; esac
for TOOL in cargo python3; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN10 same-frame composite: skipped ($TOOL absent)"; exit 0; }; done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
BUILDER=$G/delta-resolved-to-ckir5-fixture.py
PRIMARY=$G/fixtures/ckir5-payload-sums/general.omg
T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN10_COMPOSITE_TEMP:-0}" = 1 ]; then echo "OMGRFN10 same-frame composite: retained $T" >&2; else trap 'rm -rf "$T"' EXIT; fi

observe() { # label expected frame executable
  LABEL=$1 EXPECTED=$2 FRAME=$3 EXECUTABLE=$4
  set +e
  "$EXECUTABLE" < "$FRAME" > "$T/$LABEL.out" 2> "$T/$LABEL.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$LABEL.out" ] || {
    echo "OMGRFN10 same-frame composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/$LABEL.err" >&2
    exit 1
  }
}
run_pair() { observe "$4-native" "$3" "$2" "$T/$1.native"; observe "$4-self" "$3" "$2" "$T/$1.self"; }

python3 -B "$R/omgrfn10-materialize-r1-r2.py" "$T/checkers"
python3 -B "$R/omgrfn10-materialize-r3-r5.py" "$T/checkers"
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
  [ "$BYTES" -le 262140 ] || { echo "OMGRFN10 same-frame composite: $NAME tape $BYTES exceeds ceiling" >&2; exit 1; }
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.native" >/dev/null 2>&1
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.self" >/dev/null 2>&1
done

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir4.alp" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-checked-ir-v5-to-elf.alp" "$T/backend" >/dev/null

python3 -B - "$BUILDER" "$PRIMARY" "$T/source.omgc" <<'PY'
from pathlib import Path
import importlib.util, sys
helper, primary, output = map(Path, sys.argv[1:])
spec = importlib.util.spec_from_file_location("omgrfn10_fixture", helper)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)
source = primary.read_text(encoding="ascii")
gate = """data ScalarGate { word: u32; byte: u8; flag: bool; }
machine ScalarGate::check(&mut self) -> u8 {
    self.word = 70;
    self.byte = 70;
    self.flag = true;
    transition self.word == 70 { true -> word_unequal() false -> failed() }
    state word_unequal(&mut self) { transition self.word == 71 { true -> failed() false -> byte_equal() } }
    state byte_equal(&mut self) { transition self.byte == 70 { true -> byte_unequal() false -> failed() } }
    state byte_unequal(&mut self) { transition self.byte == 69 { true -> failed() false -> flag_equal() } }
    state flag_equal(&mut self) { transition self.flag == true { true -> flag_unequal() false -> failed() } }
    state flag_unequal(&mut self) { transition self.flag == false { true -> failed() false -> passed() } }
    state failed(&mut self) { 0 }
    state passed(&mut self) { 70 }
}

"""
source = source.replace("data SumProducer {", gate + "data SumProducer {", 1)
source = source.replace(
    "data SumProducer {\n    pad: u8;\n    current: Packet;\n}",
    "data SumProducer {\n    pad: u8;\n    current: Packet;\n    gate: ScalarGate;\n}",
    1,
)
old = """machine SumProducer::run(&mut self) -> u8 {
    self.current = Packet::Empty;
"""
new = """machine SumProducer::run(&mut self) -> u8 {
    self.pad = self.gate.check();
    self.current = Packet::Empty;
"""
assert source.count(old) == 1
source = source.replace(old, new, 1)
output.write_bytes(module.encode_source(source, "SumProducer", "run"))
PY
"$T/resolver" < "$T/source.omgc" > "$T/witness"
python3 -B - "$T/source.omgc" "$T/witness" "$T/input.omglow" "$BUILDER" <<'PY'
from pathlib import Path
import importlib.util, sys
c, w, out, helper = map(Path, sys.argv[1:])
spec = importlib.util.spec_from_file_location("omgrfn10_fixture", helper)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)
witness = w.read_bytes()
out.write_bytes(module.pack_lowering(c.read_bytes(), witness, 9, witness[8]))
PY
"$T/lowerer" < "$T/input.omglow" > "$T/output.ckir8"
"$T/backend" < "$T/output.ckir8" > "$T/output.elf"
python3 -B "$R/omgrfn10_bundle.py" "$T/source.omgc" "$T/witness" "$T/output.ckir8" "$T/output.elf" --result 70 > "$T/primary.rfn"

python3 -B - "$T/primary.rfn" "$T" <<'PY'
from pathlib import Path
import struct, sys
raw = Path(sys.argv[1]).read_bytes(); out = Path(sys.argv[2])
oc, ow, ck, el = struct.unpack_from("<4I", raw, 16)
ckir = 40 + oc + ow; elf = ckir + ck
def put(name, data): (out / f"{name}.rfn").write_bytes(data)
x=bytearray(raw); x[6]=ord("9"); struct.pack_into("<I",x,8,9); put("outer9",x)
x=bytearray(raw); x[ckir+8]=7; put("ckir7",x)
x=bytearray(raw); struct.pack_into("<II",x,32,71,71); put("claim71",x)
x=bytearray(raw); start=40; changed=0
while True:
    at=bytes(x).find(b"==",start,40+oc)
    if at<0: break
    x[at:at+2]=b"!="; start=at+2; changed+=1
assert changed==6; put("source-not-equal",x)
x=bytearray(raw); at=bytes(x).find(b"\x0f\x94\xc0\x0f\xb6\xc0",elf,elf+el); assert at>=0; x[at+1]=0x95; put("elf-setne",x)
counts=struct.unpack_from("<19I",raw,ckir+24); widths=(24,20,16,20,20,16,36,20,32,20,24,4,40,4,52,24,12)
cursor=ckir+100
for count,width in zip(counts[:12],widths[:12]): cursor += count*width
rows=[cursor+i*40 for i in range(counts[12]) if raw[cursor+i*40+12]==18]
assert len(rows)==6
x=bytearray(raw)
for row in rows: x[row+12]=17
put("ckir-opcode",x)
put("trailing",raw+b"\0")
PY

for NAME in $CHECKERS; do run_pair "$NAME" "$T/primary.rfn" 0 "$NAME-positive"; done
for NAME in $CHECKERS; do run_pair "$NAME" "$T/outer9.rfn" 251 "$NAME-outer9"; run_pair "$NAME" "$T/trailing.rfn" 251 "$NAME-trailing"; done
run_pair r2 "$T/source-not-equal.rfn" 251 r2-source-not-equal
run_pair r3 "$T/ckir7.rfn" 251 r3-ckir7
run_pair r3 "$T/ckir-opcode.rfn" 251 r3-ckir-opcode
run_pair r4-lowering "$T/ckir-opcode.rfn" 251 r4-ckir-opcode
run_pair r5-structure "$T/ckir-opcode.rfn" 251 r5-ckir-opcode
run_pair r5-elf "$T/elf-setne.rfn" 251 r5-elf-setne
for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-elf; do run_pair "$NAME" "$T/claim71.rfn" 0 "$NAME-claim-opacity"; done
run_pair r5-result "$T/claim71.rfn" 251 r5-result-claim

echo "OMGRFN10 same-frame composite: R1-R5 native/self scalar equality and ownership teeth passed"
