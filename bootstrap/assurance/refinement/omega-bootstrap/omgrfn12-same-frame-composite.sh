#!/usr/bin/env sh
# OMGRFN12 immutable-frame R1--R5 composition for exact u8 -> u32 widening.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P); OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT; . "$OMEGA_REPO_ROOT/bootstrap/paths.sh"; . "$OMEGA_PATH_BETA/artifact_env.sh"; . "$OMEGA_PATH_ALPHA/seed_env.sh"; cd "$OMEGA_REPO_ROOT"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "OMGRFN12 same-frame composite: skipped (requires Darwin arm64)"; exit 0;; esac
for TOOL in cargo python3; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN12 same-frame composite: skipped ($TOOL absent)"; exit 0; }; done
R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT; G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES; C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
BUILDER=$G/delta-resolved-to-ckir5-fixture.py; PRIMARY=$G/fixtures/ckir5-payload-sums/general.omg; T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN12_COMPOSITE_TEMP:-0}" = 1 ]; then echo "OMGRFN12 same-frame composite: retained $T" >&2; else trap 'rm -rf "$T"' EXIT; fi
observe(){ LABEL=$1 EXPECTED=$2 FRAME=$3 EXECUTABLE=$4; set +e; "$EXECUTABLE" < "$FRAME" > "$T/$LABEL.out" 2> "$T/$LABEL.err"; ACTUAL=$?; set -e; [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$LABEL.out" ] || { echo "OMGRFN12 same-frame composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2; sed -n '1,12p' "$T/$LABEL.err" >&2; exit 1; }; }
run_pair(){ observe "$4-native" "$3" "$2" "$T/$1.native"; observe "$4-self" "$3" "$2" "$T/$1.self"; }
python3 -B "$R/omgrfn12-materialize-r1-r2.py" "$T/checkers"; python3 -B "$R/omgrfn12-materialize-r3-r5.py" "$T/checkers"
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED; ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED; stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"; "$ASM" < "$T/bc1.asm" > "$T/bc1.tape"; stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
CHECKERS='r1 r2 r3 r4-lowering r4-source-result r5-structure r5-result r5-elf'
for NAME in $CHECKERS; do "$T/bc0" < "$T/checkers/$NAME.beta" > "$T/$NAME.native.asm"; "$T/bc1" < "$T/checkers/$NAME.beta" > "$T/$NAME.self.asm"; cmp "$T/$NAME.native.asm" "$T/$NAME.self.asm" >/dev/null; "$ASM" < "$T/$NAME.native.asm" > "$T/$NAME.tape"; BYTES=$(wc -c < "$T/$NAME.tape" | tr -d ' '); [ "$BYTES" -le 262140 ] || { echo "OMGRFN12 same-frame composite: $NAME tape $BYTES exceeds ceiling" >&2; exit 1; }; stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.native" >/dev/null 2>&1; stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.self" >/dev/null 2>&1; done
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"; DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver" >/dev/null; DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir4.alp" "$T/lowerer" >/dev/null; DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-checked-ir-v5-to-elf.alp" "$T/backend" >/dev/null
python3 -B - "$BUILDER" "$PRIMARY" "$T/source.omgc" <<'PY'
from pathlib import Path
import importlib.util,sys
helper,primary,output=map(Path,sys.argv[1:]); spec=importlib.util.spec_from_file_location("omgrfn12_fixture",helper); module=importlib.util.module_from_spec(spec); assert spec.loader is not None; spec.loader.exec_module(module)
source=primary.read_text(encoding="ascii")
gate="""data WidenGate { byte: u8; wide: u32 in Trapping; }
machine WidenGate::check(&mut self) -> u8 {
    self.byte = 0; self.wide = self.byte as u32 in Trapping;
    self.byte = 70; self.wide = self.byte as u32 in Trapping;
    self.byte = 255; self.wide = self.byte as u32 in Trapping;
    transition self.wide == 255 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}

"""
source=source.replace("data SumProducer {",gate+"data SumProducer {",1)
source=source.replace("data SumProducer {\n    pad: u8;\n    current: Packet;\n}","data SumProducer {\n    pad: u8;\n    current: Packet;\n    gate: WidenGate;\n}",1)
old="machine SumProducer::run(&mut self) -> u8 {\n    self.current = Packet::Empty;"; new="machine SumProducer::run(&mut self) -> u8 {\n    self.pad = self.gate.check();\n    self.current = Packet::Empty;"; assert source.count(old)==1; source=source.replace(old,new,1)
assert source.count(" as u32 in Trapping")==3; output.write_bytes(module.encode_source(source,"SumProducer","run"))
PY
"$T/resolver" < "$T/source.omgc" > "$T/witness"
python3 -B - "$T/source.omgc" "$T/witness" "$T/input.omglow" "$BUILDER" <<'PY'
from pathlib import Path
import importlib.util,sys
c,w,out,helper=map(Path,sys.argv[1:]); spec=importlib.util.spec_from_file_location("omgrfn12_fixture",helper); module=importlib.util.module_from_spec(spec); assert spec.loader is not None; spec.loader.exec_module(module); witness=w.read_bytes(); out.write_bytes(module.pack_lowering(c.read_bytes(),witness,11,witness[8]))
PY
"$T/lowerer" < "$T/input.omglow" > "$T/output.ckir10"; "$T/backend" < "$T/output.ckir10" > "$T/output.elf"
python3 -B "$R/omgrfn12_bundle.py" "$T/source.omgc" "$T/witness" "$T/output.ckir10" "$T/output.elf" --result 70 > "$T/primary.rfn"
python3 -B - "$T/primary.rfn" "$T" <<'PY'
from pathlib import Path
import struct,sys
raw=Path(sys.argv[1]).read_bytes(); out=Path(sys.argv[2]); oc,ow,ck,el=struct.unpack_from("<4I",raw,16); ckir=40+oc+ow; elf=ckir+ck
def put(name,data):(out/f"{name}.rfn").write_bytes(data)
x=bytearray(raw); x[6]=ord("B"); struct.pack_into("<I",x,8,11); put("outer11",x)
x=bytearray(raw); x[ckir+8]=9; put("ckir9",x)
x=bytearray(raw); struct.pack_into("<II",x,32,71,71); put("claim71",x)
x=bytearray(raw); s=bytes(x[40:40+oc]); assert s.count(b" as u32 in Trapping")==3; s=s.replace(b" as u32 in Trapping",b" as u8  in Trapping"); x[40:40+oc]=s; put("source-target",x)
x=bytearray(raw); s=bytes(x[40:40+oc]); s=s.replace(b"Trapping",b"Wrapping",1); x[40:40+oc]=s; put("source-domain",x)
counts=struct.unpack_from("<19I",raw,ckir+24); widths=(24,20,16,20,20,16,36,20,32,20,24,4,40,4,52,24,12); cursor=ckir+100
for count,width in zip(counts[:12],widths[:12]):cursor+=count*width
rows=[cursor+i*40 for i in range(counts[12]) if raw[cursor+i*40+12]==21]; assert len(rows)==3; operand_base=cursor+counts[12]*40; row=rows[0]
x=bytearray(raw); x[row+12]=20; put("ckir-opcode",x)
x=bytearray(raw); start=struct.unpack_from("<I",x,row+24)[0]; struct.pack_into("<I",x,operand_base+start*4,struct.unpack_from("<I",x,row+16)[0]); put("ckir-operand",x)
x=bytearray(raw); operand=struct.unpack_from("<I",x,operand_base+start*4)[0]; source_type=None
for candidate in rows:
    if struct.unpack_from("<I",x,candidate+16)[0]==operand: source_type=struct.unpack_from("<I",x,candidate+20)[0]
if source_type is None:
    for candidate in range(counts[12]):
        p=cursor+candidate*40
        if struct.unpack_from("<I",x,p+16)[0]==operand: source_type=struct.unpack_from("<I",x,p+20)[0]; break
assert source_type is not None; struct.pack_into("<I",x,row+20,source_type); put("ckir-result",x)
x=bytearray(raw); result_type=struct.unpack_from("<I",x,row+20)[0]; type_base=ckir+100; x[type_base+result_type*24+5]^=1; put("ckir-type",x)
x=bytearray(raw); at=bytes(x).find(b"\x0f\xb6\xc0",elf,elf+el); assert at>=0; x[at+1]=0xb7; put("elf-movzx",x)
put("trailing",raw+b"\0"); put("resource",raw+b"\0"*(4497545-len(raw)))
PY
for NAME in $CHECKERS; do run_pair "$NAME" "$T/primary.rfn" 0 "$NAME-positive"; run_pair "$NAME" "$T/outer11.rfn" 251 "$NAME-outer11"; run_pair "$NAME" "$T/trailing.rfn" 251 "$NAME-trailing"; run_pair "$NAME" "$T/resource.rfn" 252 "$NAME-resource"; done
run_pair r2 "$T/source-target.rfn" 251 r2-source-target; run_pair r4-source-result "$T/source-domain.rfn" 251 r4-source-domain
run_pair r3 "$T/ckir9.rfn" 251 r3-ckir9; run_pair r3 "$T/ckir-opcode.rfn" 251 r3-opcode; run_pair r4-lowering "$T/ckir-opcode.rfn" 251 r4-opcode; run_pair r5-structure "$T/ckir-type.rfn" 251 r5-type; run_pair r5-structure "$T/ckir-operand.rfn" 251 r5-operand; run_pair r4-lowering "$T/ckir-result.rfn" 251 r4-result; run_pair r5-elf "$T/elf-movzx.rfn" 251 r5-elf-movzx
for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-elf; do run_pair "$NAME" "$T/claim71.rfn" 0 "$NAME-claim-opacity"; done; run_pair r5-result "$T/claim71.rfn" 251 r5-result-claim
echo "OMGRFN12 same-frame composite: R1-R5 native/self exact u8-to-trapping-u32 preservation at 0/70/255 and ownership teeth passed"
