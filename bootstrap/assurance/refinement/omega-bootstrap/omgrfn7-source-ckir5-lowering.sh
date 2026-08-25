#!/usr/bin/env sh
# Focused persisted-Beta OMGRFN7 R4 source -> exact CKIR5 gate.
set -eu

STARTED=$(date +%s)
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in "Darwin arm64") ;; *) echo "OMGRFN7 R4 source lowering: skipped (requires Darwin arm64)"; exit 0;; esac
for TOOL in cargo python3; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN7 R4 source lowering: skipped ($TOOL absent)"; exit 0; }; done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
CORE=$R/omgrfn4-source-witness-independent.beta
DECL=$R/omgrfn7-source-witness-independent.beta
LOWERING=$R/omgrfn7-source-ckir5-lowering.beta
PACKER=$R/omgrfn7_bundle.py
BUILDER=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir5-fixture.py
SOURCE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir5-payload-sums/general.omg
RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp
LOWERER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir4.alp
for FILE in "$CORE" "$DECL" "$LOWERING" "$PACKER" "$BUILDER" "$SOURCE" "$RESOLVER" "$LOWERER"; do [ -f "$FILE" ] || exit 1; done

T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN7_R4_LOWERING_TEMP:-0}" = 1 ]; then echo "OMGRFN7 R4 source lowering: retained $T" >&2; else trap 'rm -rf "$T"' EXIT; fi

# The canonical product carrier has one source unit. Keep the frozen two-unit
# R2 implementation intact and specialize only this private composition.
python3 -B - "$CORE" "$T/core.beta" <<'PY'
from pathlib import Path
import sys
s=Path(sys.argv[1]).read_text(encoding="ascii")
replacements={
 "to bad when (word[500320]!=2)":"to bad when (word[500320]!=1)",
 "to bad when (word[500336]!=0) to bad when (word[500344]!=1)":"to bad when (word[500336]!=0) to bad when (word[500344]!=0)",
 "word[500368]=64 word[500376]=112 word[500384]=152 word[500392]=152":"word[500368]=64 word[500376]=112 word[500384]=132 word[500392]=132",
 "to bad when (omgrfn4_l2_comp_u32(104)!=2)":"to bad when (omgrfn4_l2_comp_u32(104)!=1)",
 "state sources { to source_row when (i<2)":"state sources { to source_row when (i<1)",
 "to bad when (bundle>=2)":"to bad when (bundle>=1)",
 "to bad when (omgrfn4_l2_comp_u32(at+12)!=2)":"to bad when (omgrfn4_l2_comp_u32(at+12)!=1)",
 "state bundle_rows { to bundle_row when (i<2)":"state bundle_rows { to bundle_row when (i<1)",
 "state source_extents { to source_extent when (i<2)":"state source_extents { to source_extent when (i<1)",
}
for old,new in replacements.items():
    expected=2 if old=="to bad when (word[500336]!=0) to bad when (word[500344]!=1)" else 1
    if s.count(old)!=expected: raise SystemExit("R4 source custody anchor: "+old)
    s=s.replace(old,new)
Path(sys.argv[2]).write_text(s,encoding="ascii")
PY

python3 -B - "$T/main.beta" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_text("proc main() { return omgrfn7_r4_lowering_check() }\n",encoding="ascii")
PY
awk '1' "$T/core.beta" "$DECL" "$LOWERING" "$T/main.beta" > "$T/all.beta"
python3 -B - "$T/all.beta" "$T/check.beta" <<'PY'
from pathlib import Path
import re,sys
s=Path(sys.argv[1]).read_text(encoding="ascii"); procs={}; order=[]
for m in re.finditer(r"(?m)^proc\s+(\w+)\s*\([^)]*\)\s*\{",s):
 d=1; p=m.end()
 while d: d+=(s[p]=="{")-(s[p]=="}"); p+=1
 name=m.group(1)
 if name in procs: raise SystemExit("duplicate "+name)
 procs[name]=s[m.start():p]+"\n"; order.append(name)
seen=set(); todo=["main"]
while todo:
 n=todo.pop()
 if n in seen: continue
 seen.add(n)
 for call in re.findall(r"\b(\w+)\s*\(",procs[n]):
  if call in procs and call not in seen: todo.append(call)
Path(sys.argv[2]).write_text("\n".join(procs[n] for n in order if n in seen),encoding="ascii")
PY
if [ -n "${OMEGA_OMGRFN7_R4_LOWERING_EXPORT:-}" ]; then
  cp "$T/check.beta" "$OMEGA_OMGRFN7_R4_LOWERING_EXPORT"
  exit 0
fi

PROCEDURES=$(awk '/^proc / {n++} END {print n+0}' "$T/check.beta")
MAX_LOCALS=$(python3 -B - "$T/check.beta" <<'PY'
import re,sys
s=open(sys.argv[1],encoding="ascii").read(); maximum=0
for m in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{",s,re.M):
 end=s.find("\nproc ",m.end()); body=s[m.end():end if end>=0 else len(s)]
 maximum=max(maximum,sum(bool(x.strip()) for x in m.group(1).split(","))+len(re.findall(r"\blet\s+[A-Za-z_]\w*",body)))
print(maximum)
PY
)
[ "$PROCEDURES" -le 128 ] && [ "$MAX_LOCALS" -le 32 ] || exit 1

SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"
"$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
"$T/bc0" < "$T/check.beta" > "$T/native.asm"
"$T/bc1" < "$T/check.beta" > "$T/self.asm"
cmp "$T/native.asm" "$T/self.asm" >/dev/null
"$ASM" < "$T/native.asm" > "$T/check.tape"
TAPE_BYTES=$(wc -c < "$T/check.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || exit 1
stamp_seed "$T/check.tape" "$SEED" "$T/native" >/dev/null 2>&1
stamp_seed "$T/check.tape" "$SEED" "$T/self" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null
python3 -B - "$SOURCE" "$T" "$BUILDER" <<'PY'
from pathlib import Path
import importlib.util,subprocess,sys
source=Path(sys.argv[1]).read_text(encoding="ascii"); temp=Path(sys.argv[2]); helper=Path(sys.argv[3])
spec=importlib.util.spec_from_file_location("producer",helper); producer=importlib.util.module_from_spec(spec); spec.loader.exec_module(producer)
comp=producer.encode_source(source); (temp/"positive.omgc").write_bytes(comp)
witness=subprocess.run([str(temp/"resolver")],input=comp,stdout=subprocess.PIPE,check=True).stdout; (temp/"positive.witness").write_bytes(witness)
ckir=subprocess.run([str(temp/"lowerer")],input=producer.pack_lowering(comp,witness),stdout=subprocess.PIPE,check=True).stdout; (temp/"positive.ckir").write_bytes(ckir)
(temp/"opaque.elf").write_bytes(b"opaque")
PY
python3 -B "$PACKER" "$T/positive.omgc" "$T/positive.witness" "$T/positive.ckir" "$T/opaque.elf" --result 70 > "$T/positive.rfn"

observe_one() { set +e; "$1" < "$3" > "$T/out" 2> "$T/err"; ACTUAL=$?; set -e; [ "$ACTUAL" -eq "$2" ] || { echo "OMGRFN7 R4 source lowering: $4 got $ACTUAL expected $2" >&2; exit 1; }; [ ! -s "$T/out" ] || exit 1; }
observe() { observe_one "$T/native" "$1" "$2" "$3 native"; observe_one "$T/self" "$1" "$2" "$3 self"; }
observe 0 "$T/positive.rfn" positive

python3 -B - "$T/positive.rfn" "$T" <<'PY'
from pathlib import Path
import struct,sys
raw=Path(sys.argv[1]).read_bytes(); temp=Path(sys.argv[2]); oc,ow,ck,el=struct.unpack_from("<4I",raw,16); w=40+oc; c=w+ow; e=c+ck
def put(name,data): (temp/name).write_bytes(data)
def source_mut(name,old,new):
 x=bytearray(raw); at=bytes(x).find(old,40,40+oc)
 if at<0 or len(old)!=len(new): raise SystemExit("mutation anchor "+name)
 x[at:at+len(old)]=new; put(name+".rfn",x)
source_mut("tail",b"tail: 70",b"tail: 71")
counts=struct.unpack_from("<19I",raw,c+24); at=c+100; widths=(24,20,16,20,20,16,36,20,32,20,24,4,40,4,52,24,12); offsets=[]
for count,width in zip(counts[:17],widths): offsets.append(at); at+=count*width
op14=next(i for i in range(counts[12]) if raw[offsets[12]+i*40+12]==14)
x=bytearray(raw); struct.pack_into("<I",x,offsets[12]+op14*40+32,1); put("construct.rfn",x)
x=bytearray(raw); struct.pack_into("<I",x,offsets[15]+12,0); put("arm.rfn",x)
x=bytearray(raw); x[e]^=1; put("elf.rfn",x)
x=bytearray(raw); struct.pack_into("<II",x,32,71,71); put("result.rfn",x)
x=bytearray(raw); x[6]=ord("6"); struct.pack_into("<I",x,8,6); put("v6.rfn",x)
put("over.rfn",raw+b"\0"*(4_497_545-len(raw)))
PY
observe 251 "$T/tail.rfn" source-literal-relation
observe 251 "$T/construct.rfn" constructor-case-relation
observe 251 "$T/arm.rfn" dispatch-arm-relation
observe 0 "$T/elf.rfn" elf-opacity
observe 0 "$T/result.rfn" result-opacity
observe 251 "$T/v6.rfn" v6-separation
observe 252 "$T/over.rfn" frame-resource-ceiling

ELAPSED=$(( $(date +%s) - STARTED ))
echo "OMGRFN7 responsibility 4 lowering: source literals, ConstructCase, Copy/Call, value+nonzero self-field CaseDispatch, payload bindings and exact CKIR5 join passed native/self"
echo "OMGRFN7 responsibility 4 lowering resources: ${PROCEDURES}/128 procedures; ${MAX_LOCALS}/32 locals; ${TAPE_BYTES}/262140 tape; elapsed=${ELAPSED}s"
