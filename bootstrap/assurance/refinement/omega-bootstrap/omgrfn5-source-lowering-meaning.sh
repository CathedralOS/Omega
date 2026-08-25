#!/usr/bin/env sh
# Focused persisted-Beta OMGRFN5/6 responsibility-4 source-lowering/meaning gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN5 responsibility 4: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN5 responsibility 4: skipped ($TOOL absent)"; exit 0; }
done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
ENVELOPE=$R/omgrfn5-component-envelope.beta
BASE=$R/omgrfn2-resolved-body-model.beta
MODEL=$R/omgrfn4-source-body-model.beta
COMMON=$R/ckir-refinement-source-lowering.beta
V3=$R/omgrfn3-resolved-body-lowering.beta
V4OPS=$R/omgrfn4-operation-lowering.beta
V4RESULT=$R/omgrfn4-source-only-result.beta
R4=$R/omgrfn5-source-lowering-meaning.beta
PACKER=$R/omgrfn5_bundle.py
PACKER6=$R/omgrfn6_bundle.py
BUILDER=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir4-fixture.py
LOW_FRAME=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir4-frame.py
RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp
LOWERER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir4.alp
BACKEND=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v4-to-elf.alp
FIXTURES=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir4-runtime-records
EXACT=$OMEGA_REPO_ROOT/compiler/psi/source/source.omg
for REQUIRED in "$ENVELOPE" "$BASE" "$MODEL" "$COMMON" "$V3" "$V4OPS" "$V4RESULT" "$R4" "$PACKER" "$PACKER6" "$BUILDER" "$LOW_FRAME" "$RESOLVER" "$LOWERER" "$BACKEND" "$EXACT" "$FIXTURES/source-unit-harness.omg" "$FIXTURES/source-unit-api-harness.omg" "$FIXTURES/source-unit-api-analogue.omg" "$FIXTURES/constant-assignment.omg" "$FIXTURES/direct-field-receiver.omg"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN5 responsibility 4: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN5_R4_TEMP:-0}" = 1 ]; then echo "OMGRFN5 responsibility 4: retained $T" >&2; else trap 'rm -rf "$T"' EXIT; fi
START_NS=$(python3 -c 'import time; print(time.time_ns())')

filter_procs() { python3 -B - "$1" "$2" "$3" <<'PY'
from pathlib import Path
import re,sys
s=Path(sys.argv[1]).read_text(encoding="ascii"); excluded=set(filter(None,sys.argv[3].split(","))); out=[]
for m in re.finditer(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*\{",s):
 d=1; p=m.end()
 while d: d+=(s[p]=="{")-(s[p]=="}"); p+=1
 if m.group(1) not in excluded: out.append(s[m.start():p].rstrip()+"\n")
Path(sys.argv[2]).write_text("\n".join(out),encoding="ascii")
PY
}
extract_proc() { python3 -B - "$1" "$2" "$3" <<'PY'
from pathlib import Path
import re,sys
s=Path(sys.argv[1]).read_text(encoding="ascii"); m=re.search(rf"(?m)^proc\s+{re.escape(sys.argv[2])}\s*\([^)]*\)\s*\{{",s)
if not m: raise SystemExit("missing "+sys.argv[2])
d=1; p=m.end()
while d: d+=(s[p]=="{")-(s[p]=="}"); p+=1
Path(sys.argv[3]).write_text(s[m.start():p]+"\n",encoding="ascii")
PY
}
prune() { python3 -B - "$1" "$2" "$3" <<'PY'
from pathlib import Path
import re,sys
s=Path(sys.argv[1]).read_text(encoding="ascii"); procs={}; order=[]
for m in re.finditer(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*\{",s):
 d=1; p=m.end()
 while d: d+=(s[p]=="{")-(s[p]=="}"); p+=1
 name=m.group(1)
 if name in procs: raise SystemExit("duplicate procedure "+name)
 procs[name]=s[m.start():p].rstrip()+"\n"; order.append(name)
seen=set(); todo=[sys.argv[3]]
while todo:
 n=todo.pop()
 if n in seen: continue
 if n not in procs: raise SystemExit("missing reachable "+n)
 seen.add(n)
 for c in re.findall(r"\b([A-Za-z_]\w*)\s*\(",procs[n]):
  if c in procs and c not in seen: todo.append(c)
Path(sys.argv[2]).write_text("\n".join(procs[n] for n in order if n in seen),encoding="ascii")
PY
}

sed 's/omgrfn2_component/omgrfn5_component/g' "$BASE" > "$T/base-all.beta"
# The shared carrier decoder records the exact frame version in word[524376].
# Specialize only this composed copy of the frozen model so the same R4
# executable admits precisely OMGRFN5/OMGRSW1 or OMGRFN6/OMGRSW2.
python3 -B - "$T/base-all.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); s=p.read_text(encoding="ascii")
old="    state magic6 { to bad when (l4_wbyte(6) != '1')  to magic7 }"
new="    state magic6 { to bad when (l4_wbyte(6) - 44 != word[524376])  to magic7 }"
if s.count(old)!=1: raise SystemExit("witness magic relation anchor")
s=s.replace(old,new)
old="        to bad when (l4_wbyte(8) != 1)"
new="        to bad when (l4_wbyte(8) + 4 != word[524376])"
if s.count(old)!=1: raise SystemExit("witness schema relation anchor")
p.write_text(s.replace(old,new),encoding="ascii")
PY
filter_procs "$T/base-all.beta" "$T/base.beta" 'l4_model_declarations,l4_model_types_records_fields,l4_model_machines_blocks,l4_model_prepare'
# R2/R3 own complete witness/table canonicity.  R4 retains the loader's exact
# projections but consumes that validated premise instead of duplicating every
# row guard in its bounded executable.
python3 -B - "$MODEL" "$T/model.beta" <<'PY'
from pathlib import Path
import sys
s=Path(sys.argv[1]).read_text(encoding="ascii"); needle="to bad when ("; out=[]; i=0
while True:
    at=s.find(needle,i)
    if at<0: out.append(s[i:]); break
    out.append(s[i:at]); p=at+len(needle); depth=1
    while depth:
        depth+=(s[p]=="(")-(s[p]==")"); p+=1
    i=p
Path(sys.argv[2]).write_text("".join(out),encoding="ascii")
PY
filter_procs "$COMMON" "$T/common.beta" 'src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_expression,src_low_transition,src_low_body,src_reconstruct_lowering_check,src_refinement_lowering_check,main'
sed '/^proc v3_ckir_header_check/,$d' "$V3" > "$T/v3-all.beta"
filter_procs "$T/v3-all.beta" "$T/v3.beta" 'v3_call_begin,v3_call_binding,v3_call_finish,src_low_emit,src_low_postfix'
extract_proc "$V4RESULT" v3_call_binding "$T/source-binding.beta"
extract_proc "$COMMON" src_low_transition "$T/guarded.beta"
python3 -B - "$T/guarded.beta" <<'PY'
from pathlib import Path
import re,sys
p=Path(sys.argv[1]); s=p.read_text(encoding="ascii")
s=s.replace("proc src_low_transition()","proc v4_guarded_transition_after_keyword()",1)
s=s.replace("state keyword { src_next()  to guard }","state keyword { to guard }",1)
p.write_text(s,encoding="ascii")
PY
extract_proc "$V4RESULT" v4_guarded_transition_after_keyword "$T/source-guarded.beta"
filter_procs "$V4OPS" "$T/v4ops.beta" 'src_low_body,omgrfn4_r4_operation_check,main'
filter_procs "$V4RESULT" "$T/v4result.beta" 'v3_call_binding,src_low_expression,v4s_guardless_transition,v4_guarded_transition_after_keyword,src_low_transition,src_low_body,main'
sed 's/v4s_parse_constant/v4_skip_constant/g' "$R4" > "$T/r4-lowering.beta"
awk '1' "$ENVELOPE" "$T/base.beta" "$T/model.beta" "$T/common.beta" "$T/v3.beta" "$T/source-binding.beta" "$T/guarded.beta" "$T/v4ops.beta" "$T/r4-lowering.beta" > "$T/lowering-all.beta"
prune "$T/lowering-all.beta" "$T/lowering.beta" main

# Build the physically artifact-free companion before dependency pruning.
sed '/^proc omgrfn5_component_ckir_byte/,$d' "$ENVELOPE" > "$T/source-envelope.beta"
filter_procs "$COMMON" "$T/source-common.beta" 'ckir_u32,ckir_row_word,ckir_row_byte,ckir_bparam_word,ckir_operand,src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_expression,src_low_transition,src_low_body,src_low_block_owner,src_lower_compare_final,src_reconstruct_lowering_check,src_refinement_lowering_check,main'
sed '/^proc v3_ckir_header_check/,$d' "$V3" > "$T/source-v3-all.beta"
filter_procs "$T/source-v3-all.beta" "$T/source-v3.beta" 'v3_call_begin,v3_call_binding,v3_call_finish,src_low_emit,src_low_postfix'
filter_procs "$R4" "$T/r4-source.beta" 'v5_ckir_header_check,omgrfn5_r4_lowering_check,main'
cat > "$T/source-lowering-main.beta" <<'EOF'
proc omgrfn5_r4_source_lowering_check() {
    let status=omgrfn5_component_read()
    state frame { to done when (status!=0) status=v4_model_prepare() to done when (status!=0) src_init_words() word[20500032]=0 to lowering }
    state lowering { status=src_reconstruct_lowering_check() to done when (status!=0) to bad when (word[20500032]+6-word[524376]==0) status=v5s_prepare_objects() to done when (status!=0) status=v5_direct_edge_check() to done }
    state bad { status=src_reject() to done }
    state done { return status }
}
proc main() { return omgrfn5_r4_source_lowering_check() }
EOF
cat > "$T/source-result-main.beta" <<'EOF'
proc omgrfn5_r4_source_result_check() {
    let status=omgrfn5_component_read()
    state frame { to done when (status!=0) status=v4_model_prepare() to done when (status!=0) src_init_words() word[20500032]=0 to lowering }
    state lowering { status=src_reconstruct_lowering_check() to done when (status!=0) to bad when (word[20500032]+6-word[524376]==0) v5s_prepare_objects() status=v4s_source_result_check() to done }
    state bad { status=src_reject() to done }
    state done { return status }
}
proc main() { return omgrfn5_r4_source_result_check() }
EOF
awk '1' "$T/source-envelope.beta" "$T/base.beta" "$T/model.beta" "$T/source-common.beta" "$T/source-v3.beta" "$T/source-binding.beta" "$T/source-guarded.beta" "$T/v4ops.beta" "$T/v4result.beta" "$T/r4-source.beta" > "$T/source-common-all.beta"
python3 -B - "$T/source-common-all.beta" <<'PY'
from pathlib import Path
import re,sys
p=Path(sys.argv[1]); s=p.read_text(encoding="ascii")
old="to less_equal when (opcode==12) to trap"
if s.count(old)!=1: raise SystemExit("source evaluator dispatch anchor")
s=s.replace(old,"to less_equal when (opcode==12) to constructor when (opcode==13) to trap")
old="    state call { callee=src_lower_op(op,8)"
if s.count(old)!=1: raise SystemExit("source evaluator call anchor")
s=s.replace(old,"    state constructor { to failed when (v5s_construct(op,depth,value_base)!=0) to op_next }\n"+old)
s=s.replace("ckir_row_word(7,src_low_g(0),40,32)","0")
s=s.replace("v4s_parse_constant(destination_type,0)","v4_skip_constant(destination_type)")
old="    state aggregate { a=src_lower_operand(start) raw=src_lower_op(op,8) address=word[place_base+a*8] to failed when (v4s_install(raw,address,0)!=0) to op_next }"
if s.count(old)!=1: raise SystemExit("source inherited aggregate anchor")
s=s.replace(old,"    state aggregate { to op_next }")
p.write_text(s,encoding="ascii")
PY
awk '1' "$T/source-common-all.beta" "$T/source-lowering-main.beta" > "$T/source-lowering-all.beta"
awk '1' "$T/source-common-all.beta" "$T/source-result-main.beta" > "$T/source-result-all.beta"
python3 -B - "$T/source-lowering-all.beta" "$T/source-result-all.beta" <<'PY'
from pathlib import Path
import sys
full="state member_refine { to member_emit when (src_low_g(5)==4294967295) to member_emit when (word[19600000+src_low_g(5)*32]!=1) to member_emit when (word[19600008+src_low_g(5)*32]!=1) to member_emit when (word[19600016+src_low_g(5)*32]!=field) to member_emit when (src_low_g(22)<=word[19600024+src_low_g(5)*32]) src_low_gset(22,word[19600024+src_low_g(5)*32]) to member_emit }"
for name in sys.argv[1:]:
    p=Path(name); s=p.read_text(encoding="ascii")
    if s.count(full)!=1: raise SystemExit("source premise member-refinement anchor in "+name)
    if s.count("to bad when (index_high>=src_type(base_type,5)) ")!=1: raise SystemExit("source premise index-bound anchor in "+name)
    p.write_text(s.replace(full,"state member_refine { to member_emit }").replace(
        "to bad when (index_high>=src_type(base_type,5)) ",""),encoding="ascii")
PY
python3 -B - "$T/source-result-all.beta" <<'PY'
from pathlib import Path
import re,sys
p=Path(sys.argv[1]); s=p.read_text(encoding="ascii")
m=re.search(r"(?m)^proc v5_scalar_self_path\(expected,mode\) \{",s)
if not m: raise SystemExit("missing source-result self-path validator")
d=1; end=m.end()
while d: d+=(s[end]=="{")-(s[end]=="}"); end+=1
lean_self='''proc v5_scalar_self_path(expected,mode) {
    let base_type=0 let base_place=0 let field=0
    state self_primary { to done when (src_low_primary()!=0) to suffix }
    state suffix { to materialize when (word[524416]!=46) base_type=src_low_g(20) base_place=src_low_g(28) src_next() to name }
    state name { field=src_low_find_field(src_type(base_type,3),word[524424],word[524432]) src_low_gset(20,src_field(field,3)) src_low_gset(21,src_type(src_field(field,3),7)) src_low_gset(22,src_type(src_field(field,3),8)) src_low_gset(23,1) src_low_gset(24,0) src_low_gset(25,1) src_low_gset(26,field) src_low_gset(50,base_place) src_low_gset(51,0) src_low_gset(52,field) to done when (src_low_emit(3,2,src_type(src_field(field,3),9),1)!=0) src_next() to suffix }
    state materialize { return src_low_materialize(src_type(expected,9)) }
    state bad { return src_reject() }
    state done { return word[10000000] }
}'''
s=s[:m.start()]+lean_self+s[end:]
m=re.search(r"(?m)^proc v5s_prepare_objects\(\) \{",s)
if not m: raise SystemExit("missing source-result object allocator")
d=1; end=m.end()
while d: d+=(s[end]=="{")-(s[end]=="}"); end+=1
lean='''proc v5s_prepare_objects() {
    let machine=0 let op=0 let cursor=0 let value=0 let size=0
    state machines { to machine_one when (machine<word[10000032]) return 0 }
    state machine_one { cursor=0 op=0 to operations }
    state operations { to operation when (op<src_low_g(0)) machine=machine+1 to machines }
    state operation { to next when (src_lower_op(op,0)!=machine) to next when (src_lower_op(op,2)!=13) value=src_lower_op(op,4) size=v4s_type_size(src_lower_op(op,5)) word[43500000+value*8]=cursor cursor=cursor+size to next }
    state next { op=op+1 to operations }
}'''
s=s[:m.start()]+lean+s[end:]
# R1--R3 own the fixed transport identities.  This physically artifact-free
# executable retains every bounded table/extent walk and the carrier/witness
# relation bytes, while omitting duplicate magic-byte state chains.
def rewrite_proc(source,name,edit):
    m=re.search(rf"(?m)^proc {name}\([^)]*\) \{{",source)
    if not m: raise SystemExit("source-result premise proc "+name)
    d=1; end=m.end()
    while d: d+=(source[end]=="{")-(source[end]=="}"); end+=1
    return source[:m.start()]+edit(source[m.start():end])+source[end:]
def lean_sources(body):
    body=body.replace("state size { to bad when (n < 80)  to magic0 }","state size { to bad when (n < 80)  to header }",1)
    for prefix in ("magic", "bundle_magic"):
        for i in range(8):
            body,n=re.subn(rf"(?m)^    state {prefix}{i} \{{[^\n]*\}}\n", "", body, count=1)
            if n!=1: raise SystemExit("source-result premise chain "+prefix+str(i))
    return body.replace("        to bundle_magic0\n", "        to bundle_header\n", 1)
def lean_witness(body):
    body=body.replace("state size { to bad when (n < 72)  to magic0 }","state size { to bad when (n < 72)  to magic6 }",1)
    for i in range(6):
        body,n=re.subn(rf"(?m)^    state magic{i} \{{[^\n]*\}}\n", "", body, count=1)
        if n!=1: raise SystemExit("source-result witness premise chain "+str(i))
    return body
s=rewrite_proc(s,"l4_decode_sources",lean_sources)
s=rewrite_proc(s,"l4_decode_witness",lean_witness)
p.write_text(s,encoding="ascii")
PY
prune "$T/source-lowering-all.beta" "$T/source-lowering.beta" main
prune "$T/source-result-all.beta" "$T/source-result.beta" main

python3 -B - "$T/source-lowering.beta" "$T/source-result.beta" <<'PY'
from pathlib import Path
import re,sys
bad={"omgrfn5_component_ckir_byte","omgrfn5_component_elf_byte","refinement_ckir_byte","refinement_elf_byte","ckir_u32","ckir_row_word","ckir_row_byte","ckir_operand","v5_ckir_header_check","src_lower_compare_final"}
for name in sys.argv[1:]:
    s=Path(name).read_text(encoding="ascii")
    used=set(re.findall(r"\b([A-Za-z_]\w*)\s*\(",s))
    if used & bad: raise SystemExit("artifact reachability in "+name+": "+repr(sorted(used&bad)))
for anchor in ("v5_direct_edge_check","src_reconstruct_lowering_check"):
    if anchor not in Path(sys.argv[1]).read_text(encoding="ascii"): raise SystemExit("missing source-lowering anchor "+anchor)
for anchor in ("opcode==13","v5s_construct","depth>=16","word[43400000]>=65536"):
    if anchor not in Path(sys.argv[2]).read_text(encoding="ascii"): raise SystemExit("missing source-result anchor "+anchor)
PY

resource_row() { # source prefix
  SOURCE_FILE=$1 PREFIX=$2
  PROC_COUNT=$(awk '/^proc / {n++} END {print n+0}' "$SOURCE_FILE")
  LOCAL_COUNT=$(python3 -B - "$SOURCE_FILE" <<'PY'
import re,sys
s=open(sys.argv[1],encoding="ascii").read(); maximum=0
for m in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{",s,re.M):
    end=s.find("\nproc ",m.end()); body=s[m.end():end if end>=0 else len(s)]
    maximum=max(maximum,sum(bool(x.strip()) for x in m.group(1).split(","))+len(re.findall(r"\blet\s+[A-Za-z_]\w*",body)))
print(maximum)
PY
)
  [ "$PROC_COUNT" -le 128 ] && [ "$LOCAL_COUNT" -le 32 ] || { echo "OMGRFN5 responsibility 4: $PREFIX source ceiling $PROC_COUNT/$LOCAL_COUNT" >&2; exit 1; }
  eval "${PREFIX}_PROCS=$PROC_COUNT"
  eval "${PREFIX}_LOCALS=$LOCAL_COUNT"
}
resource_row "$T/lowering.beta" LOWER
resource_row "$T/source-lowering.beta" SOURCE_LOWERING
resource_row "$T/source-result.beta" SOURCE_RESULT

SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"
"$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
for CHECK in lowering source-lowering source-result; do
  "$T/bc0" < "$T/$CHECK.beta" > "$T/$CHECK.native.asm"
  "$T/bc1" < "$T/$CHECK.beta" > "$T/$CHECK.self.asm"
  cmp "$T/$CHECK.native.asm" "$T/$CHECK.self.asm" >/dev/null
  "$ASM" < "$T/$CHECK.native.asm" > "$T/$CHECK.tape"
  TAPE_COUNT=$(wc -c < "$T/$CHECK.tape" | tr -d ' ')
  [ "$TAPE_COUNT" -le 262140 ] || { echo "OMGRFN5 responsibility 4: $CHECK tape $TAPE_COUNT" >&2; exit 1; }
  case "$CHECK" in
    lowering) lowering_TAPE=$TAPE_COUNT ;;
    source-lowering) source_lowering_TAPE=$TAPE_COUNT ;;
    source-result) source_result_TAPE=$TAPE_COUNT ;;
  esac
  stamp_seed "$T/$CHECK.tape" "$SEED" "$T/$CHECK" >/dev/null 2>&1
done

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend" >/dev/null

observe() { # exe input expected label
  set +e
  "$1" < "$2" > "$T/$4.out" 2> "$T/$4.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$3" ] || { echo "OMGRFN5 responsibility 4: $4=$ACTUAL expected $3" >&2; tail -20 "$T/$4.err" >&2; exit 1; }
  [ "$3" -eq 0 ] || [ ! -s "$T/$4.out" ] || { echo "OMGRFN5 responsibility 4: $4 published rejection" >&2; exit 1; }
}

build_case() { # name owner machine source...
  NAME=$1 OWNER=$2 MACHINE=$3
  shift 3
  python3 -B "$BUILDER" build "$T/$NAME.omgc" "$OWNER" "$MACHINE" "$@"
  observe "$T/resolver" "$T/$NAME.omgc" 0 "$NAME-resolver"
  mv "$T/$NAME-resolver.out" "$T/$NAME.witness"
  python3 -B "$LOW_FRAME" pack "$T/$NAME.omgc" "$T/$NAME.witness" > "$T/$NAME.low"
  observe "$T/lowerer" "$T/$NAME.low" 0 "$NAME-lowerer"
  mv "$T/$NAME-lowerer.out" "$T/$NAME.ckir"
  observe "$T/backend" "$T/$NAME.ckir" 0 "$NAME-backend"
  mv "$T/$NAME-backend.out" "$T/$NAME.elf"
  WITNESS_RELATION=$(python3 -B - "$T/$NAME.witness" <<'PY'
from pathlib import Path
import sys
b=Path(sys.argv[1]).read_bytes()
print(b[6] if len(b)>6 else 0)
PY
)
  CASE_PACKER=$PACKER
  [ "$WITNESS_RELATION" -eq 50 ] && CASE_PACKER=$PACKER6
  [ "$WITNESS_RELATION" -eq 49 ] || [ "$WITNESS_RELATION" -eq 50 ] || { echo "OMGRFN5/6 responsibility 4: $NAME unknown witness relation" >&2; exit 1; }
  python3 -B "$CASE_PACKER" "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.ckir" "$T/$NAME.elf" --result 70 > "$T/$NAME.rfn"
}

build_case exact SourceUnit bootstrap_runtime_record_probe "$EXACT" "$FIXTURES/source-unit-harness.omg"
build_case source_api SourceUnit bootstrap_source_api_probe "$EXACT" "$FIXTURES/source-unit-api-harness.omg"

# Mode-one literal records are runtime values, but the pre-existing aggregate
# assignment path remains a CKIR3 constant copy.  Keep that distinction local
# and explicit so broadening call arguments cannot silently rewrite assignment.
build_case constant ConstantPairProbe run "$FIXTURES/constant-assignment.omg"
python3 -B - "$T/constant.ckir" <<'PY'
from pathlib import Path
import struct, sys
b = Path(sys.argv[1]).read_bytes()
counts = struct.unpack_from("<14I", b, 24)
at = 80
for count, size in zip(counts[:7], (24, 20, 16, 36, 20, 32, 20)):
    at += count * size
at += counts[12] * 24 + counts[13] * 4
opcodes = [b[at + row * 40 + 12] for row in range(counts[7])]
if 11 not in opcodes or 13 in opcodes:
    raise SystemExit("constant assignment must use CopyAggregateConst without ConstructRecord")
PY
observe "$T/lowering" "$T/constant.rfn" 0 constant-lowering
observe "$T/source-lowering" "$T/constant.rfn" 0 constant-source-lowering
# OMGRFN4 owns constant-assignment source meaning; this R4 regression row does
# not enlarge the artifact-free source-result evaluator.

# This responsibility-local analogue varies every nominal/machine spelling and
# adds an unrelated fifth machine without changing the admitted API shape.
build_case renamed_api BootstrapBuffer exercise "$FIXTURES/source-unit-api-analogue.omg"
build_case authored RuntimePairProbe run "$FIXTURES/authored-declaration-order.omg"
build_case reordered RuntimePairProbe run "$FIXTURES/declaration-order.omg"
build_case nested NestedRuntimeProbe run "$FIXTURES/nested-runtime.omg"
build_case direct DirectCallProbe run "$FIXTURES/direct-call.omg"
build_case field_receiver FieldReceiverProbe run "$FIXTURES/direct-field-receiver.omg"
for CASE in exact source_api renamed_api authored reordered nested direct field_receiver; do
  observe "$T/lowering" "$T/$CASE.rfn" 0 "$CASE-lowering"
  observe "$T/source-lowering" "$T/$CASE.rfn" 0 "$CASE-source-lowering"
  observe "$T/source-result" "$T/$CASE.rfn" 0 "$CASE-source-result"
done
cmp "$T/authored.ckir" "$T/reordered.ckir" >/dev/null

# OMGRFN6 preserves byte-identical CKIR4.  Inspect both direct calls: their
# receiver is the result of FieldPlace(SelfPlace), and the selected field has a
# nonzero compact-record offset so artifact-free result 70 exercises the base.
python3 -B - "$T/field_receiver.ckir" <<'PY'
from pathlib import Path
import struct,sys
b=Path(sys.argv[1]).read_bytes(); counts=struct.unpack_from("<14I",b,24); at=80
sizes=(24,20,16,36,20,32,20); bases=[]
for count,size in zip(counts[:7],sizes): bases.append(at); at+=count*size
at+=counts[12]*24+counts[13]*4
ops=at; operands=ops+counts[7]*40
rows=[]
for i in range(counts[7]):
    row=ops+i*40
    rows.append((i,b[row+12],struct.unpack_from("<I",b,row+16)[0],struct.unpack_from("<I",b,row+24)[0],struct.unpack_from("<I",b,row+28)[0],struct.unpack_from("<I",b,row+32)[0]))
places={result:(i,opcode,start,count,imm) for i,opcode,result,start,count,imm in rows if result!=0xffffffff}
chains=[]
for i,opcode,result,start,count,target in rows:
    if opcode!=10 or count<1: continue
    receiver=struct.unpack_from("<I",b,operands+start*4)[0]; field_row=places.get(receiver)
    if not field_row or field_row[1]!=3: continue
    self_place=struct.unpack_from("<I",b,operands+field_row[2]*4)[0]; self_row=places.get(self_place)
    if not self_row or self_row[1]!=2: raise SystemExit("FieldPlace receiver is not rooted in SelfPlace")
    field=field_row[4]; offset=struct.unpack_from("<I",b,bases[2]+field*16+12)[0]
    if offset==0: raise SystemExit("field receiver must exercise a nonzero runtime offset")
    chains.append((self_row[0],field_row[0],i,self_place,receiver))
if len(chains)!=2: raise SystemExit(f"expected two SelfPlace->FieldPlace->Call chains, got {len(chains)}")
PY

# Cross-version pairs are rejected by every R4 view before source meaning.
python3 -B - "$T/exact.rfn" "$T/field_receiver.rfn" "$T/v6-with-v1.rfn" "$T/v5-with-v2.rfn" <<'PY'
from pathlib import Path
import struct,sys
v5=bytearray(Path(sys.argv[1]).read_bytes()); v5[:8]=b"OMGRFN6\0"; struct.pack_into("<I",v5,8,6); Path(sys.argv[3]).write_bytes(v5)
v6=bytearray(Path(sys.argv[2]).read_bytes()); v6[:8]=b"OMGRFN5\0"; struct.pack_into("<I",v6,8,5); Path(sys.argv[4]).write_bytes(v6)
PY
for CHECK in lowering source-lowering source-result; do
  observe "$T/$CHECK" "$T/v6-with-v1.rfn" 251 "v6-with-v1-$CHECK"
  observe "$T/$CHECK" "$T/v5-with-v2.rfn" 251 "v5-with-v2-$CHECK"
done

# Reject a parenthesized receiver even though spacing keeps every witnessed
# callee span byte-identical.  This isolates the exact direct-syntax rule.
python3 -B - "$T/field_receiver.omgc" "$T/field-shape.omgc" <<'PY'
from pathlib import Path
import sys
b=Path(sys.argv[1]).read_bytes(); old=b"    self.cell.read()"; new=b"  (self.cell).read()"
if len(old)!=len(new) or b.count(old)!=1: raise SystemExit("direct field shape mutation anchor")
Path(sys.argv[2]).write_bytes(b.replace(old,new))
PY
python3 -B "$PACKER6" "$T/field-shape.omgc" "$T/field_receiver.witness" "$T/field_receiver.ckir" "$T/field_receiver.elf" --result 70 > "$T/field-shape.rfn"
for CHECK in lowering source-lowering source-result; do observe "$T/$CHECK" "$T/field-shape.rfn" 251 "field-shape-$CHECK"; done

# CKIR-only receiver and ordering mutations are visible to the source/artifact
# join and deliberately opaque to both artifact-free source owners.
python3 -B - "$T/field_receiver.rfn" "$T/field-wrong-receiver.rfn" "$T/field-wrong-order.rfn" <<'PY'
from pathlib import Path
import struct,sys
b=bytearray(Path(sys.argv[1]).read_bytes()); omg,wit=struct.unpack_from("<2I",b,16); base=40+omg+wit
counts=struct.unpack_from("<14I",b,base+24); at=base+80
for count,size in zip(counts[:7],(24,20,16,36,20,32,20)): at+=count*size
at+=counts[12]*24+counts[13]*4
ops=at; operands=ops+counts[7]*40; places={}; chain=None
for i in range(counts[7]):
    row=ops+i*40; opcode=b[row+12]; result=struct.unpack_from("<I",b,row+16)[0]; start=struct.unpack_from("<I",b,row+24)[0]; count=struct.unpack_from("<I",b,row+28)[0]
    if result!=0xffffffff: places[result]=(i,opcode,start)
    if opcode==10 and count:
        receiver=struct.unpack_from("<I",b,operands+start*4)[0]; field=places.get(receiver)
        if field and field[1]==3:
            self_place=struct.unpack_from("<I",b,operands+field[2]*4)[0]; chain=(places[self_place][0],field[0],i,self_place,start); break
if chain is None: raise SystemExit("missing direct field call mutation anchor")
self_i,field_i,call_i,self_place,call_start=chain
wrong=bytearray(b); struct.pack_into("<I",wrong,operands+call_start*4,self_place); Path(sys.argv[2]).write_bytes(wrong)
order=bytearray(b); a=ops+self_i*40; c=ops+field_i*40; first=bytes(order[a:a+40]); second=bytes(order[c:c+40]); order[a:a+40]=second; order[c:c+40]=first; Path(sys.argv[3]).write_bytes(order)
PY
for MUTATION in field-wrong-receiver field-wrong-order; do
  observe "$T/lowering" "$T/$MUTATION.rfn" 251 "$MUTATION-lowering"
  observe "$T/source-lowering" "$T/$MUTATION.rfn" 0 "$MUTATION-source-lowering-opacity"
  observe "$T/source-result" "$T/$MUTATION.rfn" 0 "$MUTATION-source-result-opacity"
done

# The exact SourceUnit API is sensitive to ordinary call order.  This valid
# carrier clears after appending and therefore returns zero; lowering accepts
# it while the artifact-free result owner rejects the deliberately false 70.
python3 -B - "$FIXTURES/source-unit-api-harness.omg" "$T/source-unit-api-order.omg" <<'PY'
from pathlib import Path
import sys
s = Path(sys.argv[1]).read_text(encoding="ascii")
s = s.replace("bootstrap_source_api_probe", "bootstrap_source_api_order_probe")
s = s.replace(
    "        self.clear(SourceId { value: 1 });\n        self.append(70);",
    "        self.append(70);\n        self.clear(SourceId { value: 1 });",
)
Path(sys.argv[2]).write_text(s, encoding="ascii")
PY
build_case source_api_order SourceUnit bootstrap_source_api_order_probe "$EXACT" "$T/source-unit-api-order.omg"
observe "$T/lowering" "$T/source_api_order.rfn" 0 source-api-order-lowering
observe "$T/source-lowering" "$T/source_api_order.rfn" 0 source-api-order-source-lowering
observe "$T/source-result" "$T/source_api_order.rfn" 251 source-api-order-source-result

# A same-result source mutation changes the constructor literal and therefore
# the CKIR bytes without changing the final byte observation.  Pairing that
# source/witness with the original artifact must fail only the source/artifact
# join; both artifact-free R4 owners still accept it.
python3 -B - "$FIXTURES/source-unit-api-harness.omg" "$T/source-unit-api-id2.omg" <<'PY'
from pathlib import Path
import sys
s = Path(sys.argv[1]).read_text(encoding="ascii")
s = s.replace("bootstrap_source_api_probe", "bootstrap_source_api_id2_probe")
s = s.replace("SourceId { value: 1 }", "SourceId { value: 2 }")
Path(sys.argv[2]).write_text(s, encoding="ascii")
PY
build_case source_api_id2 SourceUnit bootstrap_source_api_id2_probe "$EXACT" "$T/source-unit-api-id2.omg"
python3 -B "$PACKER" "$T/source_api_id2.omgc" "$T/source_api_id2.witness" "$T/source_api.ckir" "$T/source_api.elf" --result 70 > "$T/source-api-cross.rfn"
observe "$T/lowering" "$T/source-api-cross.rfn" 251 source-api-cross-lowering
observe "$T/source-lowering" "$T/source-api-cross.rfn" 0 source-api-cross-source-lowering
observe "$T/source-result" "$T/source-api-cross.rfn" 0 source-api-cross-source-result

# Valid cross-pair and phase-local opacity controls.
python3 -B "$PACKER" "$T/authored.omgc" "$T/authored.witness" "$T/nested.ckir" "$T/nested.elf" --result 70 > "$T/cross.rfn"
observe "$T/lowering" "$T/cross.rfn" 251 cross-lowering
cp "$T/exact.rfn" "$T/opaque.rfn"
python3 -B - "$T/opaque.rfn" <<'PY'
from pathlib import Path
import struct,sys
p=Path(sys.argv[1]); b=bytearray(p.read_bytes()); omg,wit,ck,elf=struct.unpack_from("<4I",b,16); base=40+omg+wit
b[base:base+ck]=bytes([165])*ck; b[base+ck:base+ck+elf]=bytes([90])*elf; p.write_bytes(b)
PY
observe "$T/source-lowering" "$T/opaque.rfn" 0 source-lowering-artifact-opacity
observe "$T/source-result" "$T/opaque.rfn" 0 source-result-artifact-opacity

python3 -B - "$T/authored.rfn" "$T/op.rfn" "$T/result.rfn" <<'PY'
from pathlib import Path
import struct,sys
b=bytearray(Path(sys.argv[1]).read_bytes()); omg,wit=struct.unpack_from("<2I",b,16); base=40+omg+wit; counts=struct.unpack_from("<14I",b,base+24); at=base+80
for count,size in zip(counts[:7],(24,20,16,36,20,32,20)): at+=count*size
at+=counts[12]*24+counts[13]*4
for i in range(counts[7]):
    if b[at+i*40+12]==13: b[at+i*40+12]=12; break
Path(sys.argv[2]).write_bytes(b)
r=bytearray(Path(sys.argv[1]).read_bytes()); struct.pack_into("<I",r,32,71); struct.pack_into("<I",r,36,71); Path(sys.argv[3]).write_bytes(r)
PY
observe "$T/lowering" "$T/op.rfn" 251 opcode13-mutation
observe "$T/source-lowering" "$T/op.rfn" 0 source-lowering-ckir-opacity
observe "$T/source-result" "$T/op.rfn" 0 source-result-ckir-opacity
observe "$T/lowering" "$T/result.rfn" 0 lowering-result-opacity
observe "$T/source-lowering" "$T/result.rfn" 0 source-lowering-result-opacity
observe "$T/source-result" "$T/result.rfn" 251 source-result-mutation

# Semantic validity precedes the four/five constructor resource decision.
python3 -B - "$T" <<'PY'
from pathlib import Path
import sys
t=Path(sys.argv[1])
def source(n,body=None):
    fields="\n".join(f"    f{i}: u8;" for i in range(n))
    authored=body or ", ".join(f"f{i}: {'self.scalar' if i==0 else i}" for i in range(n))
    return f"data R [copy] {{\n{fields}\n}}\ndata P {{ value: R; scalar: u8; }}\nmachine P::run(&mut self) -> u8 {{ self.scalar = 70; self.value = R {{ {authored} }}; self.scalar }}\n"
(t/"four.omg").write_text(source(4),encoding="ascii")
(t/"five.omg").write_text(source(5),encoding="ascii")
(t/"five-bad.omg").write_text(source(5,"f0: self.scalar, f0: 1, f1: 1, f2: 2, f3: 3"),encoding="ascii")
PY
build_case four P run "$T/four.omg"
observe "$T/lowering" "$T/four.rfn" 0 four-lowering
observe "$T/source-lowering" "$T/four.rfn" 0 four-source-lowering
observe "$T/source-result" "$T/four.rfn" 0 four-source-result
for CASE in five five-bad; do
  python3 -B "$BUILDER" build "$T/$CASE.omgc" P run "$T/$CASE.omg"
  observe "$T/resolver" "$T/$CASE.omgc" 0 "$CASE-resolver"
  mv "$T/$CASE-resolver.out" "$T/$CASE.witness"
  python3 -B "$LOW_FRAME" pack "$T/$CASE.omgc" "$T/$CASE.witness" > "$T/$CASE.low"
  EXPECT=252
  [ "$CASE" = five-bad ] && EXPECT=251
  observe "$T/lowerer" "$T/$CASE.low" "$EXPECT" "$CASE-product"
  python3 -B "$PACKER" "$T/$CASE.omgc" "$T/$CASE.witness" "$T/authored.ckir" "$T/authored.elf" --result 70 > "$T/$CASE.rfn"
  observe "$T/lowering" "$T/$CASE.rfn" "$EXPECT" "$CASE-r4"
  observe "$T/source-lowering" "$T/$CASE.rfn" "$EXPECT" "$CASE-source-lowering"
  observe "$T/source-result" "$T/$CASE.rfn" "$EXPECT" "$CASE-source-result"
done

# Frozen carriers reject one another; malformed source remains 251 before any
# oversized opaque component can grant a relation conclusion.
cp "$T/exact.rfn" "$T/v4.rfn"
python3 -B - "$T/v4.rfn" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); b=bytearray(p.read_bytes()); b[:8]=b"OMGRFN4\0"; b[8]=4; p.write_bytes(b)
PY
observe "$T/lowering" "$T/v4.rfn" 251 v4-separation
observe "$T/source-lowering" "$T/v4.rfn" 251 v4-source-lowering-separation
observe "$T/source-result" "$T/v4.rfn" 251 v4-source-result-separation

END_NS=$(python3 -c 'import time; print(time.time_ns())')
TOTAL_MS=$(((END_NS-START_NS)/1000000))
echo "OMGRFN5/6 responsibility 4: v5 exact/API/renamed carriers 0/0/0; v6 direct self.field carrier has two SelfPlace->nonzero FieldPlace->Call chains and result 70 at 0/0/0; v5/v6 cross-version and exact-shape negatives 251/251/251; wrong receiver/order CKIR 251/0/0; existing negatives passed native/self"
echo "OMGRFN5/6 responsibility 4 resources: lowering ${LOWER_PROCS}/128 procs ${LOWER_LOCALS}/32 locals ${lowering_TAPE}/262140 tape; source-lowering ${SOURCE_LOWERING_PROCS}/128 procs ${SOURCE_LOWERING_LOCALS}/32 locals ${source_lowering_TAPE}/262140 tape; source-result ${SOURCE_RESULT_PROCS}/128 procs ${SOURCE_RESULT_LOCALS}/32 locals ${source_result_TAPE}/262140 tape; total=${TOTAL_MS}ms"
