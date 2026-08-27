#!/usr/bin/env sh
# Same-exact-frame composition of all five independent OMGRFN7 duties.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
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
  *) echo "OMGRFN7 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN7 same-frame composite: skipped ($TOOL absent)"
    exit 0
  }
done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
R1CORE=$R/omgrfn4-frame-omgcomp-custody.beta
R1=$R/omgrfn5-frame-omgcomp-custody.beta
R2CORE=$R/omgrfn4-source-witness-independent.beta
R2=$R/omgrfn7-source-witness-independent.beta
R3=$R/omgrfn7-witness-ckir5-tables.beta
R4LOWERING_GATE=$R/omgrfn7-source-ckir5-lowering.sh
R4RESULT_GATE=$R/omgrfn7-source-lowering-meaning.sh
R5STRUCTURE_GATE=$R/omgrfn7-ckir5-structure.sh
R5RESULT_GATE=$R/omgrfn7-ckir5-result.sh
R5ELF_GATE=$R/omgrfn7-ckir5-elf.sh
PACKER=$R/omgrfn7_bundle.py
PACKER5=$R/omgrfn5_bundle.py
PACKER6=$R/omgrfn6_bundle.py
FIXTURE=$G/delta-resolved-to-ckir5-fixture.py
SOURCE=$G/fixtures/ckir5-payload-sums/general.omg
IR5=$G/checked_ir_v5_reference.py
IR4=$G/checked_ir_v4_reference.py
LOWFRAME4=$G/delta-resolved-to-ckir4-frame.py
RESOLVER=$C/omega-bootstrap-resolve.alp
LOWERER=$C/omega-bootstrap-resolved-to-ckir4.alp
BACKEND4=$C/omega-bootstrap-checked-ir-v4-to-elf.alp
BACKEND5=$C/omega-bootstrap-checked-ir-v5-to-elf.alp
CHECKERS='r1 r2 r3 r4-lowering r4-source-result r5-structure r5-result r5-elf'

for REQUIRED in "$R1CORE" "$R1" "$R2CORE" "$R2" "$R3" \
  "$R4LOWERING_GATE" "$R4RESULT_GATE" \
  "$R5STRUCTURE_GATE" "$R5RESULT_GATE" "$R5ELF_GATE" "$PACKER" \
  "$PACKER5" "$PACKER6" "$FIXTURE" "$SOURCE" "$IR5" "$IR4" \
  "$LOWFRAME4" "$RESOLVER" "$LOWERER" "$BACKEND4" "$BACKEND5" \
  "$OMEGA_PATH_BETA/bc.beta"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN7 same-frame composite: missing $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_OMGRFN7_COMPOSITE_TEMP:-0}" = 1 ]; then
    echo "OMGRFN7 same-frame composite: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT
: > "$T/timings.tsv"

python3 - "$T/run.py" <<'PY'
from pathlib import Path
import os, signal, subprocess, sys, time
Path(sys.argv[1]).write_text(r'''#!/usr/bin/env python3
from pathlib import Path
import os,signal,subprocess,sys,time
label,expected,timeout,source,output,empty,timings,*command=sys.argv[1:]
started=time.monotonic()
with open(source,"rb") as inp:
    process=subprocess.Popen(command,stdin=inp,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    try: stdout,stderr=process.communicate(timeout=float(timeout))
    except subprocess.TimeoutExpired:
        os.killpg(process.pid,signal.SIGKILL); stdout,stderr=process.communicate()
        raise SystemExit(f"{label} exceeded {timeout}s")
elapsed=time.monotonic()-started
Path(output).write_bytes(stdout); Path(output+".stderr").write_bytes(stderr)
with open(timings,"a",encoding="ascii") as report: report.write(f"{elapsed:.6f}\t{label}\n")
if process.returncode!=int(expected):
    if stderr: sys.stderr.buffer.write(stderr[-4096:])
    raise SystemExit(f"{label} returned {process.returncode}, expected {expected}")
if empty=="yes" and stdout: raise SystemExit(f"{label} published {len(stdout)} bytes")
''',encoding="ascii")
PY

observe() { # label status timeout stdin stdout require-empty command...
  LABEL=$1 STATUS=$2 TIMEOUT=$3 INPUT=$4 OUTPUT=$5 EMPTY=$6
  shift 6
  python3 "$T/run.py" "$LABEL" "$STATUS" "$TIMEOUT" "$INPUT" "$OUTPUT" \
    "$EMPTY" "$T/timings.tsv" "$@"
}
wait_all() {
  STATUS=0
  set +e
  for PID in "$@"; do
    wait "$PID"
    [ "$?" -eq 0 ] || STATUS=1
  done
  set -e
  return "$STATUS"
}

# Responsibility programs share implementation sources but retain independent
# native/self-built executables and verdicts over the same immutable bytes.
cat "$R1CORE" "$R1" > "$T/r1.beta"
printf '\nproc main() { return omgrfn7_layer1_check() }\n' >> "$T/r1.beta"
# The canonical payload-sum product is a one-source OMGCOMP. Specialize only
# this private composition; the shared frozen two-unit R2 implementation and
# its focused fixture remain byte-identical.
python3 -B - "$R2CORE" "$R2" "$T/r2-core.beta" "$T/r2-decl.beta" <<'PY'
from pathlib import Path
import sys
core=Path(sys.argv[1]).read_text(encoding="ascii")
decl=Path(sys.argv[2]).read_text(encoding="ascii")
core_replacements={
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
for old,new in core_replacements.items():
 expected=2 if old=="to bad when (word[500336]!=0) to bad when (word[500344]!=1)" else 1
 if core.count(old)!=expected: raise SystemExit("R2 source custody anchor: "+old)
 core=core.replace(old,new)
decl_replacements={
 "expected=84+2*36+word[879072]*28":"expected=84+1*36+word[879072]*28",
 "omgrfn4_l2_put_u32(expected) omgrfn4_l2_put_u32(2)":"omgrfn4_l2_put_u32(expected) omgrfn4_l2_put_u32(1)",
 "state units { to unit when (i<2)":"state units { to unit when (i<1)",
 "state tokenize { to tokenize_one when (source<2)":"state tokenize { to tokenize_one when (source<1)",
 "state parse { to parse_one when (source<2)":"state parse { to parse_one when (source<1)",
}
for old,new in decl_replacements.items():
 if decl.count(old)!=1: raise SystemExit("R2 one-source anchor: "+old)
 decl=decl.replace(old,new)
Path(sys.argv[3]).write_text(core,encoding="ascii")
Path(sys.argv[4]).write_text(decl,encoding="ascii")
PY
cat "$T/r2-core.beta" "$T/r2-decl.beta" > "$T/r2.beta"
printf '\nproc main() { return omgrfn5_r2_check() }\n' >> "$T/r2.beta"
python3 -B - "$T/r2.beta" "$T/r2-pruned.beta" <<'PY'
from pathlib import Path
import re, sys

source = Path(sys.argv[1]).read_text(encoding="ascii")
procedures = {}
order = []
for match in re.finditer(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*\{", source):
    depth = 1
    cursor = match.end()
    while depth:
        depth += (source[cursor] == "{") - (source[cursor] == "}")
        cursor += 1
    name = match.group(1)
    if name in procedures:
        raise SystemExit("duplicate procedure " + name)
    procedures[name] = source[match.start():cursor].rstrip() + "\n"
    order.append(name)
reachable = set()
pending = ["main"]
while pending:
    name = pending.pop()
    if name in reachable:
        continue
    if name not in procedures:
        raise SystemExit("missing reachable procedure " + name)
    reachable.add(name)
    for called in re.findall(r"\b([A-Za-z_]\w*)\s*\(", procedures[name]):
        if called in procedures and called not in reachable:
            pending.append(called)
Path(sys.argv[2]).write_text(
    "\n".join(procedures[name] for name in order if name in reachable),
    encoding="ascii",
)
PY
mv "$T/r2-pruned.beta" "$T/r2.beta"
cp "$R3" "$T/r3.beta"

# R4 deliberately supplies two independent conclusions: exact source->CKIR5
# lowering and source-only meaning/result. Their focused gates export the exact
# independently dependency-pruned persisted programs used here.
OMEGA_OMGRFN7_R4_LOWERING_EXPORT="$T/r4-lowering.beta" "$R4LOWERING_GATE"
OMEGA_OMGRFN7_R4_EXPORT="$T/r4-source-result.beta" "$R4RESULT_GATE"

# R5 is three fresh-frame conjuncts. Structure owns complete CKIR5 validity;
# result and ELF independently consume the same immutable carrier and own only
# their additional meaning/artifact relations.
OMEGA_OMGRFN7_R5_STRUCTURE_EXPORT="$T/r5-structure.beta" "$R5STRUCTURE_GATE"
OMEGA_OMGRFN7_R5_RESULT_EXPORT="$T/r5-result.beta" "$R5RESULT_GATE"
OMEGA_OMGRFN7_R5_ELF_EXPORT="$T/r5-elf.beta" "$R5ELF_GATE"

# Phase opacity is a physical reachability property, not merely a mutation
# convention. Keep these checks local to the generated responsibility programs.
python3 - "$T/r1.beta" "$T/r2.beta" "$T/r3.beta" \
  "$T/r4-lowering.beta" "$T/r4-source-result.beta" \
  "$T/r5-structure.beta" "$T/r5-result.beta" "$T/r5-elf.beta" <<'PY'
from pathlib import Path
import re,sys
texts={Path(p).stem:Path(p).read_text(encoding="ascii") for p in sys.argv[1:]}
def procedures(text): return set(re.findall(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\(",text))
for name in ("r1","r2"):
    if any(x in texts[name] for x in ("refinement_ckir_byte","refinement_elf_byte")):
        raise SystemExit(name+" gained artifact access")
if "refinement_elf_byte" in texts["r4-lowering"] or "refinement_elf_byte" in texts["r4-source-result"]:
    raise SystemExit("R4 gained ELF access")
if any(x in texts["r4-source-result"] for x in ("refinement_ckir_byte","component_ckir_byte")):
    raise SystemExit("source-result gained CKIR access")
for name in ("r5-structure","r5-result","r5-elf"):
    if any(x in texts[name] for x in ("component_omgcomp_byte","component_witness_byte","l4_source_byte","l4_wbyte")):
        raise SystemExit(name+" gained source/witness access")
for name,text in texts.items():
    if "main" not in procedures(text): raise SystemExit(name+" lacks main")
PY

SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
observe beta-self-source 0 90 "$OMEGA_PATH_BETA/bc.beta" "$T/bc1.asm" no "$T/bc0"
observe beta-self-assemble 0 60 "$T/bc1.asm" "$T/bc1.tape" no "$ASM"
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1

build_checker() {
  NAME=$1
  PROCS=$(awk '/^proc / { n++ } END { print n+0 }' "$T/$NAME.beta")
  LOCALS=$(python3 -B - "$T/$NAME.beta" <<'PY'
import re,sys
s=open(sys.argv[1],encoding="ascii").read(); maximum=0
for m in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{",s,re.M):
    end=s.find("\nproc ",m.end()); body=s[m.end():end if end>=0 else len(s)]
    maximum=max(maximum,sum(bool(x.strip()) for x in m.group(1).split(","))+len(re.findall(r"\blet\s+[A-Za-z_]\w*",body)))
print(maximum)
PY
)
  [ "$PROCS" -le 128 ] && [ "$LOCALS" -le 32 ] || {
    echo "OMGRFN7 composite: $NAME shape $PROCS/$LOCALS" >&2
    return 1
  }
  observe "build-$NAME-native" 0 120 "$T/$NAME.beta" "$T/$NAME.native.asm" no "$T/bc0"
  observe "build-$NAME-self" 0 120 "$T/$NAME.beta" "$T/$NAME.self.asm" no "$T/bc1"
  cmp "$T/$NAME.native.asm" "$T/$NAME.self.asm" >/dev/null
  observe "assemble-$NAME-native" 0 90 "$T/$NAME.native.asm" "$T/$NAME.native.tape" no "$ASM"
  observe "assemble-$NAME-self" 0 90 "$T/$NAME.self.asm" "$T/$NAME.self.tape" no "$ASM"
  cmp "$T/$NAME.native.tape" "$T/$NAME.self.tape" >/dev/null
  TAPE=$(wc -c < "$T/$NAME.native.tape" | tr -d ' ')
  [ "$TAPE" -le 262140 ] || {
    echo "OMGRFN7 composite: $NAME tape $TAPE" >&2
    return 1
  }
  stamp_seed "$T/$NAME.native.tape" "$SEED" "$T/$NAME.native" >/dev/null 2>&1
  stamp_seed "$T/$NAME.self.tape" "$SEED" "$T/$NAME.self" >/dev/null 2>&1
  printf '%s\t%s\t%s\t%s\n' "$NAME" "$PROCS" "$LOCALS" "$TAPE" > "$T/$NAME.resources"
}
PIDS=''
for NAME in $CHECKERS; do
  build_checker "$NAME" &
  PIDS="$PIDS $!"
done
wait_all $PIDS

# Build two independently complete V7 products. The nearby source adds one
# semantically harmless constructor so all relational cross-pair components
# are distinct while both products retain exact result 70.
observe cargo-build 0 120 /dev/null "$T/cargo.out" yes cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
PIDS=''
for SPEC in "resolver:$RESOLVER" "lowerer:$LOWERER" \
  "backend4:$BACKEND4" "backend5:$BACKEND5"; do
  NAME=${SPEC%%:*}
  INPUT=${SPEC#*:}
  observe "compile-$NAME" 0 90 /dev/null "$T/compile-$NAME.out" yes env DELTA_ARCH=aarch64 "$DELTA" "$INPUT" "$T/$NAME" &
  PIDS="$PIDS $!"
done
wait_all $PIDS
cp "$SOURCE" "$T/exact.omg"
python3 - "$SOURCE" "$T/nearby.omg" <<'PY'
from pathlib import Path
import sys
s=Path(sys.argv[1]).read_text(encoding="ascii")
needle="    self.current = Packet::Empty;\n"
if s.count(needle)!=1: raise SystemExit("nearby constructor anchor")
Path(sys.argv[2]).write_text(s.replace(needle,needle+needle,1),encoding="ascii")
PY

build_product() {
  NAME=$1
  python3 -B - "$FIXTURE" "$T/$NAME.omg" "$T/$NAME.omgc" <<'PY'
import importlib.util,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location("fixture",sys.argv[1]); f=importlib.util.module_from_spec(spec); spec.loader.exec_module(f)
Path(sys.argv[3]).write_bytes(f.encode_source(Path(sys.argv[2]).read_text(encoding="ascii")))
PY
  observe "$NAME-resolver" 0 45 "$T/$NAME.omgc" "$T/$NAME.witness" no "$T/resolver"
  python3 -B - "$FIXTURE" "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.low6" <<'PY'
import importlib.util,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location("fixture",sys.argv[1]); f=importlib.util.module_from_spec(spec); spec.loader.exec_module(f)
Path(sys.argv[4]).write_bytes(f.pack_lowering(Path(sys.argv[2]).read_bytes(),Path(sys.argv[3]).read_bytes()))
PY
  observe "$NAME-lowerer" 0 60 "$T/$NAME.low6" "$T/$NAME.ckir5" no "$T/lowerer"
  observe "$NAME-ir-validate" 0 30 /dev/null "$T/$NAME.ir-valid" no python3 -B "$IR5" validate "$T/$NAME.ckir5"
  observe "$NAME-ir-run" 0 30 /dev/null "$T/$NAME.result" no python3 -B "$IR5" run "$T/$NAME.ckir5"
  [ "$(tr -d '\n' < "$T/$NAME.result")" = 70 ] || {
    echo "OMGRFN7 composite: $NAME result drift" >&2
    exit 1
  }
  observe "$NAME-backend" 0 90 "$T/$NAME.ckir5" "$T/$NAME.elf" no "$T/backend5"
  observe "$NAME-pack" 0 20 /dev/null "$T/$NAME.rfn" no python3 -B "$PACKER" \
    "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.ckir5" "$T/$NAME.elf" --result 70
}
build_product exact
build_product nearby
python3 -B - "$FIXTURE" "$T/exact.ckir5" <<'PY'
import importlib.util,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location("fixture",sys.argv[1]); f=importlib.util.module_from_spec(spec); spec.loader.exec_module(f)
f.inspect_positive(Path(sys.argv[2]).read_bytes())
PY
for COMPONENT in omgc witness ckir5 elf; do
  cmp -s "$T/exact.$COMPONENT" "$T/nearby.$COMPONENT" && {
    echo "OMGRFN7 composite: nearby $COMPONENT is not distinct" >&2
    exit 1
  }
done

# Produce live V5/SW1 and V6/SW2 controls through the same shared resolver and
# lowerer. They remain independently valid CKIR4/result-70 products while all
# V7 relation owners (except deliberately opaque R1) reject them.
python3 - "$T/legacy-v5.omg" "$T/legacy-v6.omg" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_text('''data LegacyV5 {}
machine LegacyV5::run(&mut self) -> u8 { 70 }
''',encoding="ascii")
Path(sys.argv[2]).write_text('''data LegacyLeaf { value: u8; }
machine LegacyLeaf::read(&self) -> u8 { 70 }
data LegacyV6 { prefix: u8; leaf: LegacyLeaf; }
machine LegacyV6::run(&mut self) -> u8 { self.leaf.read() }
''',encoding="ascii")
PY
build_legacy() {
  NAME=$1 OWNER=$2 PACK=$3 EXPECTED_MAGIC=$4
  python3 -B - "$FIXTURE" "$T/$NAME.omg" "$T/$NAME.omgc" "$OWNER" <<'PY'
import importlib.util,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location("fixture",sys.argv[1]); f=importlib.util.module_from_spec(spec); spec.loader.exec_module(f)
Path(sys.argv[3]).write_bytes(f.encode_source(Path(sys.argv[2]).read_text(encoding="ascii"),sys.argv[4],"run"))
PY
  observe "$NAME-resolver" 0 45 "$T/$NAME.omgc" "$T/$NAME.witness" no "$T/resolver"
  python3 - "$T/$NAME.witness" "$EXPECTED_MAGIC" <<'PY'
from pathlib import Path
import sys
if Path(sys.argv[1]).read_bytes()[:8] != sys.argv[2].encode("ascii")+b"\0":
    raise SystemExit("legacy resolver identity drift")
PY
  observe "$NAME-low-frame" 0 20 /dev/null "$T/$NAME.low" no python3 -B "$LOWFRAME4" \
    pack "$T/$NAME.omgc" "$T/$NAME.witness"
  observe "$NAME-lowerer" 0 60 "$T/$NAME.low" "$T/$NAME.ckir4" no "$T/lowerer"
  observe "$NAME-ir-validate" 0 30 /dev/null "$T/$NAME.ir-valid" no python3 -B "$IR4" validate "$T/$NAME.ckir4"
  observe "$NAME-ir-run" 0 30 /dev/null "$T/$NAME.result" no python3 -B "$IR4" run "$T/$NAME.ckir4"
  [ "$(tr -d '\n' < "$T/$NAME.result")" = 70 ] || exit 1
  observe "$NAME-backend" 0 90 "$T/$NAME.ckir4" "$T/$NAME.elf" no "$T/backend4"
  observe "$NAME-pack" 0 20 /dev/null "$T/$NAME.rfn" no python3 -B "$PACK" \
    "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.ckir4" "$T/$NAME.elf" --result 70
}
build_legacy legacy-v5 LegacyV5 "$PACKER5" OMGRSW1
build_legacy legacy-v6 LegacyV6 "$PACKER6" OMGRSW2
python3 - "$T/exact.rfn" "$T/exact.sha256" <<'PY'
from pathlib import Path
import hashlib,sys
Path(sys.argv[2]).write_text(hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest()+"\n",encoding="ascii")
PY

check() {
  NAME=$1 ROUTE=$2 EXPECTED=$3 INPUT=$4 CASE=$5
  observe "check-$NAME-$ROUTE-$CASE" "$EXPECTED" 90 "$INPUT" \
    "$T/$NAME-$ROUTE-$CASE.out" yes "$T/$NAME.$ROUTE"
}
for NAME in $CHECKERS; do
  check "$NAME" native 0 "$T/exact.rfn" exact
  check "$NAME" self 0 "$T/exact.rfn" exact
done

# Create valid adjacent-component cross-pairs, isolated local mutations, whole
# component opacity controls, both prior-version separation frames, and the
# common resource tooth without consulting any checker verdict.
python3 - "$T/exact.rfn" "$T/nearby.rfn" "$T" <<'PY'
from pathlib import Path
import hashlib,struct,sys
exact=Path(sys.argv[1]).read_bytes(); nearby=Path(sys.argv[2]).read_bytes(); out=Path(sys.argv[3])
H=struct.Struct("<8s8I")
def parts(raw):
    magic,version,flags,on,wn,cn,en,result,projection=H.unpack_from(raw)
    at=40; omgc=raw[at:at+on]; at+=on; witness=raw[at:at+wn]; at+=wn
    ckir=raw[at:at+cn]; at+=cn; elf=raw[at:at+en]
    return magic,version,flags,omgc,witness,ckir,elf,result
def pack(omgc,witness,ckir,elf,result,magic=b"OMGRFN7\0",version=7):
    return H.pack(magic,version,1,len(omgc),len(witness),len(ckir),len(elf),result,result&255)+omgc+witness+ckir+elf
_,_,_,eo,ew,ec,ee,_=parts(exact); _,_,_,no,nw,nc,ne,_=parts(nearby)
rows={
 "source-witness":pack(eo,nw,ec,ee,70),
 "witness-ckir":pack(eo,ew,nc,ne,70),
 "ckir-elf":pack(eo,ew,ec,ne,70),
 "result-pair":pack(eo,ew,ec,ee,71),
 "r1-opaque":pack(eo,b"OMGRSW3\0\x03\0\0\0",b"OMGCKIR\0\x05\0\0\0",b"opaque",71),
 "r2-opaque":pack(eo,ew,b"OMGCKIR\0\x05\0\0\0",b"opaque",71),
 "r3-opaque":pack(b"opaque source",ew,ec,b"opaque",71),
 "r4-lowering-opaque":pack(eo,ew,ec,b"opaque",71),
 "r4-result-opaque":pack(eo,ew,b"OMGCKIR\0\x05\0\0\0",b"opaque",70),
 "r5-opaque":pack(b"opaque source",b"OMGRSW3\0\x03\0\0\0",ec,ee,70),
 "source-byte":pack(no,ew,ec,ee,70),
 "as-v5":pack(eo,ew,ec,ee,70,b"OMGRFN5\0",5),
 "as-v6":pack(eo,ew,ec,ee,70,b"OMGRFN6\0",6),
}
changed=bytearray(eo); anchor=b"tail: 70"; at=bytes(changed).find(anchor)
if at<0: raise SystemExit("source-result mutation anchor")
changed[at:at+len(anchor)]=b"tail: 71"
rows["source-result-byte"]=pack(bytes(changed),ew,ec,ee,70)
magic,version,flags,on,wn,cn,en,result,projection=H.unpack_from(exact)
w=40+on; c=w+wn; e=c+cn
x=bytearray(exact); x[w+64]^=1; rows["witness-byte"]=bytes(x)
x=bytearray(exact); x[c+36]^=1; rows["ckir-byte"]=bytes(x)
x=bytearray(exact); x[e+en-1]^=1; rows["elf-byte"]=bytes(x)
for name,raw in rows.items(): (out/(name+".rfn")).write_bytes(raw)
(out/"frame-over.rfn").write_bytes(exact+b"\0"*(4_497_545-len(exact)))
PY

# Ownership joins. Each adjacent component is independently valid, so a 251
# belongs to the intended relation rather than malformed framing.
for ROUTE in native self; do
  check r1 "$ROUTE" 0 "$T/source-witness.rfn" source-witness-opaque
  check r2 "$ROUTE" 251 "$T/source-witness.rfn" source-witness
  check r3 "$ROUTE" 0 "$T/source-witness.rfn" source-body-opaque
  check r4-lowering "$ROUTE" 251 "$T/source-witness.rfn" source-witness
  check r4-source-result "$ROUTE" 0 "$T/source-witness.rfn" witness-opaque
  for NAME in r5-structure r5-result r5-elf; do check "$NAME" "$ROUTE" 0 "$T/source-witness.rfn" source-witness-opaque; done

  for NAME in r1 r2 r4-source-result r5-structure r5-result r5-elf; do check "$NAME" "$ROUTE" 0 "$T/witness-ckir.rfn" witness-ckir-opaque; done
  check r3 "$ROUTE" 0 "$T/witness-ckir.rfn" body-difference-outside-r3
  check r4-lowering "$ROUTE" 251 "$T/witness-ckir.rfn" witness-ckir

  for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-result; do check "$NAME" "$ROUTE" 0 "$T/ckir-elf.rfn" ckir-elf-opaque; done
  check r5-elf "$ROUTE" 251 "$T/ckir-elf.rfn" ckir-elf

  for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-elf; do check "$NAME" "$ROUTE" 0 "$T/result-pair.rfn" result-opaque; done
  check r5-result "$ROUTE" 251 "$T/result-pair.rfn" result
done

# Phase-local mutations and whole-component physical opacity.
for NAME in r1 r3 r4-source-result r5-structure r5-result r5-elf; do check "$NAME" native 0 "$T/source-byte.rfn" harmless-source-opaque-or-equivalent; done
for NAME in r2 r4-lowering; do check "$NAME" native 251 "$T/source-byte.rfn" harmless-source-relation; done
for NAME in r1 r2 r3 r5-structure r5-result r5-elf; do check "$NAME" native 0 "$T/source-result-byte.rfn" source-meaning-opaque; done
for NAME in r4-lowering r4-source-result; do check "$NAME" native 251 "$T/source-result-byte.rfn" source-meaning; done
for NAME in r1 r4-source-result; do check "$NAME" native 0 "$T/witness-byte.rfn" witness-byte-opaque; done
for NAME in r2 r3 r4-lowering; do check "$NAME" native 251 "$T/witness-byte.rfn" witness-byte; done
for NAME in r5-structure r5-result r5-elf; do check "$NAME" native 0 "$T/witness-byte.rfn" witness-byte-opaque; done
for NAME in r1 r2 r4-source-result; do check "$NAME" native 0 "$T/ckir-byte.rfn" ckir-byte-opaque; done
for NAME in r3 r4-lowering r5-structure r5-result r5-elf; do check "$NAME" native 251 "$T/ckir-byte.rfn" ckir-byte; done
for NAME in r1 r2 r3 r4-lowering r4-source-result r5-structure r5-result; do check "$NAME" native 0 "$T/elf-byte.rfn" elf-byte-opaque; done
check r5-elf native 251 "$T/elf-byte.rfn" elf-byte

check r1 native 0 "$T/r1-opaque.rfn" later-components
check r2 native 0 "$T/r2-opaque.rfn" artifact-result
check r3 native 0 "$T/r3-opaque.rfn" source-elf-result
check r4-lowering native 0 "$T/r4-lowering-opaque.rfn" elf-result
check r4-source-result native 0 "$T/r4-result-opaque.rfn" artifacts
check r5-structure native 0 "$T/r5-opaque.rfn" source-witness-result-elf
check r5-result native 0 "$T/r5-opaque.rfn" source-witness
check r5-elf native 0 "$T/r5-opaque.rfn" source-witness

for VERSION in v5 v6; do
  check r1 native 0 "$T/as-$VERSION.rfn" "$VERSION-later-components-opaque"
  for NAME in r2 r3 r4-lowering r4-source-result r5-structure r5-result r5-elf; do
    check "$NAME" native 251 "$T/as-$VERSION.rfn" "$VERSION-cross-pair"
  done
done
for VERSION in v5 v6; do
  check r1 native 0 "$T/legacy-$VERSION.rfn" "$VERSION-live-positive"
  for NAME in r2 r3 r4-lowering r4-source-result r5-structure r5-result r5-elf; do
    check "$NAME" native 251 "$T/legacy-$VERSION.rfn" "$VERSION-live-separation"
  done
done
for NAME in $CHECKERS; do
  check "$NAME" native 252 "$T/frame-over.rfn" frame-over
done

python3 - "$T/exact.rfn" "$T/exact.sha256" <<'PY'
from pathlib import Path
import hashlib,sys
actual=hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest()
expected=Path(sys.argv[2]).read_text(encoding="ascii").strip()
if actual!=expected: raise SystemExit("OMGRFN7 composite immutable frame changed")
PY

for NAME in $CHECKERS; do cat "$T/$NAME.resources"; done > "$T/resources.tsv"
python3 - "$T/resources.tsv" "$T/timings.tsv" "$T/exact.rfn" <<'PY'
from pathlib import Path
import sys
resources=[]
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    name,procs,locals_,tape=line.split("\t"); resources.append(f"{name}={procs}p/{locals_}l/{tape}b")
timings=[]
for line in Path(sys.argv[2]).read_text(encoding="ascii").splitlines():
    seconds,label=line.split("\t",1); timings.append((float(seconds),label))
print("OMGRFN7 same-frame composite resources: "+" ".join(resources))
print("OMGRFN7 same-frame composite slowest: "+", ".join(f"{label}:{seconds:.3f}s" for seconds,label in sorted(timings,reverse=True)[:5]))
print(f"OMGRFN7 same-frame composite: all five responsibilities accepted one immutable {Path(sys.argv[3]).stat().st_size}-byte result-70 payload-sum frame native/self; arities 0..4, nested aggregate payload, ConstructCase, Copy/Call, parameter/nonzero-self-field CaseDispatch and selected bindings; joins, opacity, mutations, V5/V6 separation, and 0/251/252 passed")
PY
