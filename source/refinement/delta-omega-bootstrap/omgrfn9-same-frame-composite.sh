#!/usr/bin/env sh
# OMGRFN9 immutable-frame R1--R5 composition for pure Boolean && and ||.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in "Darwin arm64") ;; *) echo "OMGRFN9 same-frame composite: skipped (requires Darwin arm64)"; exit 0;; esac
for TOOL in cargo python3; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN9 same-frame composite: skipped ($TOOL absent)"; exit 0; }; done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
PRIMARY=$G/fixtures/ckir7-logical-binary/general.omg
BUILDER=$G/delta-resolved-to-ckir5-fixture.py
PACKER=$R/omgrfn9_bundle.py
T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN9_COMPOSITE_TEMP:-0}" = 1 ]; then echo "OMGRFN9 same-frame composite: retained $T" >&2; else trap 'rm -rf "$T"' EXIT; fi

observe() { # label expected frame executable
  LABEL=$1 EXPECTED=$2 FRAME=$3 EXECUTABLE=$4
  set +e
  "$EXECUTABLE" < "$FRAME" > "$T/$LABEL.out" 2> "$T/$LABEL.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$LABEL.out" ] || {
    echo "OMGRFN9 same-frame composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/$LABEL.err" >&2
    exit 1
  }
}
run_pair() { # checker frame status label
  observe "$4-native" "$3" "$2" "$T/$1.native"
  observe "$4-self" "$3" "$2" "$T/$1.self"
}

python3 -B "$R/omgrfn9-materialize-r1-r2.py" "$T/checkers"
python3 -B "$R/omgrfn9-materialize-r3-r5.py" "$T/checkers"
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
  [ "$BYTES" -le 262140 ] || { echo "OMGRFN9 same-frame composite: $NAME tape $BYTES exceeds ceiling" >&2; exit 1; }
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.native" >/dev/null 2>&1
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.self" >/dev/null 2>&1
done

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir4.alp" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-checked-ir-v5-to-elf.alp" "$T/backend" >/dev/null

python3 -B - "$PRIMARY" "$BUILDER" "$T" <<'PY'
from pathlib import Path
import importlib.util, sys

primary, helper, out = map(Path, sys.argv[1:])
spec = importlib.util.spec_from_file_location("omgrfn9_fixture", helper)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)
sources = {
    "primary": (primary.read_text(encoding="ascii"), "SumProducer", "run"),
    "sw1": ("""data BoolOne {}
machine BoolOne::run(&mut self) -> u8 {
    transition false || true { true -> passed() false -> failed() }
    state failed(&mut self) { 0 }
    state passed(&mut self) { 70 }
}
""", "BoolOne", "run"),
    "sw2": ("""data BoolGate {}
data BoolHost { gate: BoolGate; }
machine BoolGate::check(&self) -> u8 {
    transition true && false { true -> failed() false -> passed() }
    state failed(&self) { 0 }
    state passed(&self) { 70 }
}
machine BoolHost::run(&mut self) -> u8 { self.gate.check() }
""", "BoolHost", "run"),
    "impure": ("""data Impure {}
machine Impure::effect(&self) -> bool { true }
machine Impure::run(&mut self) -> u8 {
    transition self.effect() || true { true -> passed() false -> failed() }
    state failed(&mut self) { 0 }
    state passed(&mut self) { 70 }
}
""", "Impure", "run"),
    "no-op": ("""data Plain {}
machine Plain::run(&mut self) -> u8 { 70 }
""", "Plain", "run"),
}
for name, (source, owner, machine) in sources.items():
    (out / f"{name}.omg").write_text(source, encoding="ascii")
    (out / f"{name}.omgc").write_bytes(module.encode_source(source, owner, machine))
PY

for NAME in primary sw1 sw2; do
  "$T/resolver" < "$T/$NAME.omgc" > "$T/$NAME.witness"
  python3 -B - "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.omglow" "$BUILDER" <<'PY'
from pathlib import Path
import importlib.util, sys
c, w, out, helper = map(Path, sys.argv[1:])
spec = importlib.util.spec_from_file_location("omgrfn9_fixture", helper)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)
witness = w.read_bytes()
out.write_bytes(module.pack_lowering(c.read_bytes(), witness, 8, witness[8]))
PY
  "$T/lowerer" < "$T/$NAME.omglow" > "$T/$NAME.ckir7"
  "$T/backend" < "$T/$NAME.ckir7" > "$T/$NAME.elf"
  python3 -B "$PACKER" "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.ckir7" "$T/$NAME.elf" --result 70 > "$T/$NAME.rfn"
done
for NAME in impure no-op; do "$T/resolver" < "$T/$NAME.omgc" > "$T/$NAME.witness"; done

python3 -B "$PACKER" "$T/primary.omgc" "$T/sw1.witness" "$T/primary.ckir7" "$T/primary.elf" --result 70 > "$T/source-witness-cross.rfn"
python3 -B "$PACKER" "$T/primary.omgc" "$T/primary.witness" "$T/sw1.ckir7" "$T/sw1.elf" --result 70 > "$T/witness-ckir-cross.rfn"
python3 -B "$PACKER" "$T/primary.omgc" "$T/primary.witness" "$T/primary.ckir7" "$T/sw1.elf" --result 70 > "$T/ckir-elf-cross.rfn"
python3 -B "$PACKER" "$T/impure.omgc" "$T/impure.witness" "$T/primary.ckir7" "$T/primary.elf" --result 70 > "$T/purity-escape.rfn"
python3 -B "$PACKER" "$T/no-op.omgc" "$T/no-op.witness" "$T/primary.ckir7" "$T/primary.elf" --result 70 > "$T/source-no-op.rfn"

python3 -B - "$T/primary.rfn" "$T" <<'PY'
from pathlib import Path
import struct, sys

raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
oc, ow, ck, el = struct.unpack_from("<4I", raw, 16)
witness = 40 + oc
ckir = witness + ow
elf = ckir + ck
def put(name, data): (out / f"{name}.rfn").write_bytes(data)

x = bytearray(raw); x[6] = ord("8"); struct.pack_into("<I", x, 8, 8); put("outer8", x)
x = bytearray(raw); x[ckir + 8] = 6; put("ckir6", x)
x = bytearray(raw); struct.pack_into("<II", x, 32, 71, 71); put("claim71", x)
x = bytearray(raw); struct.pack_into("<II", x, 32, 326, 70); put("claim326", x)
x = bytearray(raw); at = bytes(x).find(b"false || true", 40, witness); assert at >= 0; x[at+6:at+8] = b"&&"; put("source-op-swap", x)
x = bytearray(raw); at = bytes(x).find(b"true || false && false || false", 40, witness); assert at >= 0; x[at+5:at+7] = b"&&"; put("source-precedence", x)

counts = struct.unpack_from("<19I", raw, ckir + 24)
widths = (24,20,16,20,20,16,36,20,32,20,24,4,40,4,52,24,12)
base=[]; cursor=ckir+100
for count,width in zip(counts[:17],widths): base.append(cursor); cursor += count*width
binary=[i for i in range(counts[12]) if raw[base[12]+i*40+12] in (16,17)]
assert len(binary) >= 2
row=base[12]+binary[0]*40
x=bytearray(raw); x[row+12]=16 if x[row+12]==17 else 17; put("ckir-op-swap",x)
x=bytearray(raw); struct.pack_into("<I",x,row+28,1); put("ckir-arity",x)
x=bytearray(raw); struct.pack_into("<I",x,row+32,1); put("ckir-immediate",x)
x=bytearray(raw); struct.pack_into("<I",x,row+20,0); put("ckir-type",x)
x=bytearray(raw); a=base[12]+binary[0]*40; b=base[12]+binary[1]*40; first=bytes(x[a:a+40]); x[a:a+40]=x[b:b+40]; x[b:b+40]=first; put("ckir-order",x)

x=bytearray(raw); at=bytes(x).find(b"\x23\x85",elf,elf+el); assert at>=0; x[at]^=1; put("elf-and",x)
x=bytearray(raw); at=bytes(x).find(b"\x0b\x85",elf,elf+el); assert at>=0; x[at]^=1; put("elf-or",x)
x=bytearray(raw); struct.pack_into("<I",x,16,267281); put("resource-omgcomp",x)
put("trailing",raw+b"\0")
PY

for NAME in $CHECKERS; do run_pair "$NAME" "$T/primary.rfn" 0 "$NAME-positive"; done
for CONTROL in sw1 sw2; do run_pair r1 "$T/$CONTROL.rfn" 0 "r1-$CONTROL"; run_pair r2 "$T/$CONTROL.rfn" 0 "r2-$CONTROL"; done
for NAME in $CHECKERS; do run_pair "$NAME" "$T/outer8.rfn" 251 "$NAME-outer8"; run_pair "$NAME" "$T/trailing.rfn" 251 "$NAME-trailing"; run_pair "$NAME" "$T/resource-omgcomp.rfn" 252 "$NAME-resource"; done

run_pair r1 "$T/source-no-op.rfn" 0 r1-no-op-opacity
run_pair r2 "$T/source-no-op.rfn" 251 r2-no-op-required
run_pair r4-source-result "$T/purity-escape.rfn" 251 r4-purity-escape
run_pair r4-lowering "$T/source-op-swap.rfn" 251 r4-source-op-swap
run_pair r4-lowering "$T/source-precedence.rfn" 251 r4-source-precedence
for MUTATION in ckir-op-swap ckir-order; do run_pair r4-lowering "$T/$MUTATION.rfn" 251 "r4-$MUTATION"; done
for MUTATION in ckir-arity ckir-immediate ckir-type; do run_pair r3 "$T/$MUTATION.rfn" 251 "r3-$MUTATION"; run_pair r5-structure "$T/$MUTATION.rfn" 251 "r5-$MUTATION"; done
run_pair r3 "$T/ckir6.rfn" 251 r3-ckir6
run_pair r5-elf "$T/elf-and.rfn" 251 r5-elf-and
run_pair r5-elf "$T/elf-or.rfn" 251 r5-elf-or
run_pair r2 "$T/source-witness-cross.rfn" 251 r2-source-witness-cross
run_pair r3 "$T/witness-ckir-cross.rfn" 251 r3-witness-ckir-cross
run_pair r5-elf "$T/ckir-elf-cross.rfn" 251 r5-ckir-elf-cross
for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-elf; do run_pair "$NAME" "$T/claim71.rfn" 0 "$NAME-claim-opacity"; done
run_pair r5-result "$T/claim71.rfn" 251 r5-result-claim
run_pair r5-result "$T/claim326.rfn" 251 r5-result-full-result

echo "OMGRFN9 same-frame composite: R1-R5 native/self primary, least SW1/SW2 controls, logical operator/precedence/purity/cross-pair/version/resource teeth passed"
