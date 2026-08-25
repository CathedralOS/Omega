#!/usr/bin/env sh
# Same-exact-frame composition of all five independent OMGRFN5 duties.
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
  *) echo "OMGRFN5 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN5 same-frame composite: skipped ($TOOL absent)"; exit 0; }; done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
ENVELOPE=$R/omgrfn5-component-envelope.beta
R1=$R/omgrfn5-frame-omgcomp-custody.beta
R1CORE=$R/omgrfn4-frame-omgcomp-custody.beta
R2=$R/omgrfn5-source-witness-independent.beta
R2CORE=$R/omgrfn4-source-witness-independent.beta
R3=$R/omgrfn5-witness-ckir4-tables.beta
R4=$R/omgrfn5-source-lowering-meaning.beta
R4BASE=$R/omgrfn2-resolved-body-model.beta
R4MODEL=$R/omgrfn4-source-body-model.beta
R4COMMON=$R/ckir-refinement-source-lowering.beta
R4V3=$R/omgrfn3-resolved-body-lowering.beta
R4OPS=$R/omgrfn4-operation-lowering.beta
R4RESULT=$R/omgrfn4-source-only-result.beta
R5ARTIFACT=$R/ckir4-refinement-artifact.beta
R5RESULT=$R/ckir4-refinement-result.beta
R5ELF=$R/ckir4-refinement-elf.beta
PACKER=$R/omgrfn5_bundle.py
BUILDER=$G/delta-resolved-to-ckir4-fixture.py
LOW_FRAME=$G/delta-resolved-to-ckir4-frame.py
IR_REFERENCE=$G/checked_ir_v4_reference.py
ELF_REFERENCE=$G/checked_elf_v4_reference.py
RESOLVER=$C/omega-bootstrap-resolve.alp
LOWERER=$C/omega-bootstrap-resolved-to-ckir4.alp
BACKEND=$C/omega-bootstrap-checked-ir-v4-to-elf.alp
SOURCE=$OMEGA_REPO_ROOT/compiler/psi/source/source.omg
FIXTURES=$G/fixtures/ckir4-runtime-records
CHECKERS='r1 r2 r3 r4-lowering r4-source-lowering r4-source-result r5-result r5-elf'
for REQUIRED in "$ENVELOPE" "$R1" "$R1CORE" "$R2" "$R2CORE" "$R3" "$R4" "$R4BASE" "$R4MODEL" "$R4COMMON" "$R4V3" "$R4OPS" "$R4RESULT" "$R5ARTIFACT" "$R5RESULT" "$R5ELF" "$PACKER" "$BUILDER" "$LOW_FRAME" "$IR_REFERENCE" "$ELF_REFERENCE" "$RESOLVER" "$LOWERER" "$BACKEND" "$SOURCE" "$FIXTURES/source-unit-harness.omg" "$FIXTURES/authored-declaration-order.omg"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN5 same-frame composite: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_OMGRFN5_COMPOSITE_TEMP:-0}" = 1 ]; then
    echo "OMGRFN5 same-frame composite: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT
: > "$T/timings.tsv"
python3 - "$T/started" <<'PY'
from pathlib import Path
import sys,time
Path(sys.argv[1]).write_text(f"{time.time():.9f}\n",encoding="ascii")
PY

# Every subprocess is process-group bounded and rejection must publish nothing.
python3 - "$T/run.py" <<'PY'
from pathlib import Path
import sys
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
  OBS_LABEL=$1 OBS_STATUS=$2 OBS_TIMEOUT=$3 OBS_INPUT=$4 OBS_OUTPUT=$5 OBS_EMPTY=$6
  shift 6
  python3 "$T/run.py" "$OBS_LABEL" "$OBS_STATUS" "$OBS_TIMEOUT" "$OBS_INPUT" "$OBS_OUTPUT" "$OBS_EMPTY" "$T/timings.tsv" "$@"
}
wait_all() { # reap every background child before reporting failure
  WAIT_STATUS=0
  set +e
  for WAIT_PID in "$@"; do
    wait "$WAIT_PID"
    CHILD_STATUS=$?
    [ "$CHILD_STATUS" -eq 0 ] || WAIT_STATUS=1
  done
  set -e
  return "$WAIT_STATUS"
}

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
 name=todo.pop()
 if name in seen: continue
 if name not in procs: raise SystemExit("missing reachable "+name)
 seen.add(name)
 for called in re.findall(r"\b([A-Za-z_]\w*)\s*\(",procs[name]):
  if called in procs and called not in seen: todo.append(called)
Path(sys.argv[2]).write_text("\n".join(procs[name] for name in order if name in seen),encoding="ascii")
PY
}

# R1-R3 are already responsibility-local persisted programs.
cat "$R1CORE" "$R1" > "$T/r1.beta"
printf '\nproc main() { return omgrfn5_layer1_check() }\n' >> "$T/r1.beta"
cat "$R2CORE" "$R2" > "$T/r2.beta"
printf '\nproc main() { return omgrfn5_r2_check() }\n' >> "$T/r2.beta"
cp "$R3" "$T/r3.beta"

# Reproduce the focused R4 compositions, including the physically artifact-free
# source evaluator. The R4 fragment owns its source/witness adapters locally.
sed 's/omgrfn2_component/omgrfn5_component/g' "$R4BASE" > "$T/r4-base-all.beta"
filter_procs "$T/r4-base-all.beta" "$T/r4-base.beta" 'l4_model_declarations,l4_model_types_records_fields,l4_model_machines_blocks,l4_model_prepare'
python3 -B - "$R4MODEL" "$T/r4-model.beta" <<'PY'
from pathlib import Path
import sys
s=Path(sys.argv[1]).read_text(encoding="ascii"); needle="to bad when ("; out=[]; i=0
while True:
 at=s.find(needle,i)
 if at<0: out.append(s[i:]); break
 out.append(s[i:at]); p=at+len(needle); depth=1
 while depth: depth+=(s[p]=="(")-(s[p]==")"); p+=1
 i=p
Path(sys.argv[2]).write_text("".join(out),encoding="ascii")
PY
filter_procs "$R4COMMON" "$T/r4-common.beta" 'src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_expression,src_low_transition,src_low_body,src_reconstruct_lowering_check,src_refinement_lowering_check,main'
sed '/^proc v3_ckir_header_check/,$d' "$R4V3" > "$T/r4-v3-all.beta"
filter_procs "$T/r4-v3-all.beta" "$T/r4-v3.beta" 'v3_call_begin,v3_call_binding,v3_call_finish,src_low_emit,src_low_postfix'
extract_proc "$R4RESULT" v3_call_binding "$T/r4-source-binding.beta"
extract_proc "$R4COMMON" src_low_transition "$T/r4-guarded.beta"
python3 -B - "$T/r4-guarded.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); s=p.read_text(encoding="ascii")
s=s.replace("proc src_low_transition()","proc v4_guarded_transition_after_keyword()",1)
s=s.replace("state keyword { src_next()  to guard }","state keyword { to guard }",1)
p.write_text(s,encoding="ascii")
PY
extract_proc "$R4RESULT" v4_guarded_transition_after_keyword "$T/r4-source-guarded.beta"
filter_procs "$R4OPS" "$T/r4-v4ops.beta" 'src_low_body,omgrfn4_r4_operation_check,main'
filter_procs "$R4RESULT" "$T/r4-v4result.beta" 'v3_call_binding,src_low_expression,v4s_guardless_transition,v4_guarded_transition_after_keyword,src_low_transition,src_low_body,main'
sed 's/v4s_parse_constant/v4_skip_constant/g' "$R4" > "$T/r4-fragment.beta"
awk '1' "$ENVELOPE" "$T/r4-base.beta" "$T/r4-model.beta" "$T/r4-common.beta" "$T/r4-v3.beta" "$T/r4-source-binding.beta" "$T/r4-guarded.beta" "$T/r4-v4ops.beta" "$T/r4-fragment.beta" > "$T/r4-lowering-all.beta"
prune "$T/r4-lowering-all.beta" "$T/r4-lowering.beta" main

sed '/^proc omgrfn5_component_ckir_byte/,$d' "$ENVELOPE" > "$T/r4-source-envelope.beta"
filter_procs "$R4COMMON" "$T/r4-source-common.beta" 'ckir_u32,ckir_row_word,ckir_row_byte,ckir_bparam_word,ckir_operand,src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_expression,src_low_transition,src_low_body,src_low_block_owner,src_lower_compare_final,src_reconstruct_lowering_check,src_refinement_lowering_check,main'
sed '/^proc v3_ckir_header_check/,$d' "$R4V3" > "$T/r4-source-v3-all.beta"
filter_procs "$T/r4-source-v3-all.beta" "$T/r4-source-v3.beta" 'v3_call_begin,v3_call_binding,v3_call_finish,src_low_emit,src_low_postfix'
filter_procs "$R4" "$T/r4-source-fragment.beta" 'v5_ckir_header_check,omgrfn5_r4_lowering_check,main'
python3 - "$T/r4-source-lowering-main.beta" "$T/r4-source-result-main.beta" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_text('''proc omgrfn5_r4_source_lowering_check() {
    let status=omgrfn5_component_read()
    state frame { to done when (status!=0) status=v4_model_prepare() to done when (status!=0) src_init_words() to lowering }
    state lowering { status=src_reconstruct_lowering_check() to done when (status!=0) status=v5s_prepare_objects() to done when (status!=0) status=v5_direct_edge_check() to done }
    state done { return status }
}
proc main() { return omgrfn5_r4_source_lowering_check() }
''',encoding="ascii")
Path(sys.argv[2]).write_text('''proc omgrfn5_r4_source_result_check() {
    let status=omgrfn5_component_read()
    state frame { to done when (status!=0) status=v4_model_prepare() to done when (status!=0) src_init_words() to lowering }
    state lowering { status=src_reconstruct_lowering_check() to done when (status!=0) v5s_prepare_objects() status=v4s_source_result_check() to done }
    state done { return status }
}
proc main() { return omgrfn5_r4_source_result_check() }
''',encoding="ascii")
PY
awk '1' "$T/r4-source-envelope.beta" "$T/r4-base.beta" "$T/r4-model.beta" "$T/r4-source-common.beta" "$T/r4-source-v3.beta" "$T/r4-source-binding.beta" "$T/r4-source-guarded.beta" "$T/r4-v4ops.beta" "$T/r4-v4result.beta" "$T/r4-source-fragment.beta" > "$T/r4-source-common-all.beta"
python3 -B - "$T/r4-source-common-all.beta" <<'PY'
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
awk '1' "$T/r4-source-common-all.beta" "$T/r4-source-lowering-main.beta" > "$T/r4-source-lowering-all.beta"
awk '1' "$T/r4-source-common-all.beta" "$T/r4-source-result-main.beta" > "$T/r4-source-result-all.beta"
python3 -B - "$T/r4-source-lowering-all.beta" "$T/r4-source-result-all.beta" <<'PY'
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
python3 -B - "$T/r4-source-result-all.beta" <<'PY'
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
p.write_text(s[:m.start()]+lean+s[end:],encoding="ascii")
PY
prune "$T/r4-source-lowering-all.beta" "$T/r4-source-lowering.beta" main
prune "$T/r4-source-result-all.beta" "$T/r4-source-result.beta" main

# R5 result and ELF owners repeat the complete CKIR4 checker independently.
sed '/^proc main()/,$d' "$R5ARTIFACT" > "$T/r5-artifact-prefix.beta"
cat "$ENVELOPE" "$T/r5-artifact-prefix.beta" "$R5RESULT" > "$T/r5-result.beta"
cat "$ENVELOPE" "$T/r5-artifact-prefix.beta" "$R5ELF" > "$T/r5-elf.beta"

# Assert phase-local physical opacity before compiling.
python3 -B - "$T/r4-source-lowering.beta" "$T/r4-source-result.beta" "$T/r5-result.beta" "$T/r5-elf.beta" <<'PY'
from pathlib import Path
import re,sys
source_bad={"omgrfn5_component_ckir_byte","omgrfn5_component_elf_byte","refinement_ckir_byte","refinement_elf_byte","ckir_u32","ckir_row_word","ckir_row_byte","ckir_operand","v5_ckir_header_check","src_lower_compare_final"}
for source_path in map(Path,sys.argv[1:3]):
 source=source_path.read_text(encoding="ascii"); used=set(re.findall(r"\b([A-Za-z_]\w*)\s*\(",source))
 if used & source_bad: raise SystemExit(f"source artifact reachability in {source_path.name}: {sorted(used&source_bad)!r}")
lowering=Path(sys.argv[1]).read_text(encoding="ascii")
for anchor in ("v5_direct_edge_check","src_reconstruct_lowering_check"):
 if anchor not in lowering: raise SystemExit("source lowering lost "+anchor)
source=Path(sys.argv[2]).read_text(encoding="ascii")
for anchor in ("opcode==13","v5s_construct","depth>=16","word[43400000]>=65536"):
 if anchor not in source: raise SystemExit("source evaluator lost "+anchor)
for path in map(Path,sys.argv[3:]):
 text=path.read_text(encoding="ascii")
 for forbidden in ("component_omgcomp_byte","component_witness_byte","l4_source_byte","l4_wbyte"):
  if forbidden in text: raise SystemExit(f"artifact owner {path.name} retained {forbidden}")
PY

# Reproduce the persisted Beta compiler through one self-host step.
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
observe beta-self-source 0 90 "$OMEGA_PATH_BETA/bc.beta" "$T/bc1.asm" no "$T/bc0"
observe beta-self-assemble 0 60 "$T/bc1.asm" "$T/bc1.tape" no "$ASM"
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1

build_checker() { # name
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
  [ "$PROCS" -le 128 ] && [ "$LOCALS" -le 32 ] || { echo "OMGRFN5 composite: $NAME shape $PROCS/$LOCALS" >&2; return 1; }
  observe "build-$NAME-native" 0 120 "$T/$NAME.beta" "$T/$NAME.native.asm" no "$T/bc0"
  observe "build-$NAME-self" 0 120 "$T/$NAME.beta" "$T/$NAME.self.asm" no "$T/bc1"
  cmp "$T/$NAME.native.asm" "$T/$NAME.self.asm" >/dev/null
  observe "assemble-$NAME-native" 0 90 "$T/$NAME.native.asm" "$T/$NAME.native.tape" no "$ASM"
  observe "assemble-$NAME-self" 0 90 "$T/$NAME.self.asm" "$T/$NAME.self.tape" no "$ASM"
  cmp "$T/$NAME.native.tape" "$T/$NAME.self.tape" >/dev/null
  TAPE=$(wc -c < "$T/$NAME.native.tape" | tr -d ' ')
  [ "$TAPE" -le 262140 ] || { echo "OMGRFN5 composite: $NAME tape $TAPE" >&2; return 1; }
  stamp_seed "$T/$NAME.native.tape" "$SEED" "$T/$NAME.native" >/dev/null 2>&1
  stamp_seed "$T/$NAME.self.tape" "$SEED" "$T/$NAME.self" >/dev/null 2>&1
  printf '%s\t%s\t%s\t%s\n' "$NAME" "$PROCS" "$LOCALS" "$TAPE" > "$T/$NAME.resources"
}
PIDS=''
for NAME in $CHECKERS; do build_checker "$NAME" & PIDS="$PIDS $!"; done
wait_all $PIDS

# Build the exact SourceUnit+harness product and one independently valid nearby
# product used only for component cross-pairs.
observe cargo-build 0 120 /dev/null "$T/cargo.out" yes cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
PIDS=''
for SPEC in "resolver:$RESOLVER" "lowerer:$LOWERER" "backend:$BACKEND"; do
  NAME=${SPEC%%:*}; INPUT=${SPEC#*:}
  observe "compile-$NAME" 0 90 /dev/null "$T/compile-$NAME.out" yes env DELTA_ARCH=aarch64 "$DELTA" "$INPUT" "$T/$NAME" & PIDS="$PIDS $!"
done
wait_all $PIDS

build_product() { # label owner machine source...
  BP_LABEL=$1 BP_OWNER=$2 BP_MACHINE=$3; shift 3
  observe "$BP_LABEL-builder" 0 30 /dev/null "$T/$BP_LABEL.builder" yes python3 -B "$BUILDER" build "$T/$BP_LABEL.omgc" "$BP_OWNER" "$BP_MACHINE" "$@"
  observe "$BP_LABEL-resolver" 0 45 "$T/$BP_LABEL.omgc" "$T/$BP_LABEL.witness" no "$T/resolver"
  observe "$BP_LABEL-frame" 0 20 /dev/null "$T/$BP_LABEL.low4" no python3 -B "$LOW_FRAME" pack "$T/$BP_LABEL.omgc" "$T/$BP_LABEL.witness"
  observe "$BP_LABEL-lowerer" 0 60 "$T/$BP_LABEL.low4" "$T/$BP_LABEL.ckir4" no "$T/lowerer"
  observe "$BP_LABEL-backend" 0 90 "$T/$BP_LABEL.ckir4" "$T/$BP_LABEL.elf" no "$T/backend"
  observe "$BP_LABEL-ir" 0 30 /dev/null "$T/$BP_LABEL.ir" no python3 -B "$IR_REFERENCE" validate "$T/$BP_LABEL.ckir4"
  observe "$BP_LABEL-result" 0 30 /dev/null "$T/$BP_LABEL.result" no python3 -B "$IR_REFERENCE" run "$T/$BP_LABEL.ckir4"
  observe "$BP_LABEL-elf" 0 30 /dev/null "$T/$BP_LABEL.elf-check" no python3 -B "$ELF_REFERENCE" check "$T/$BP_LABEL.ckir4" "$T/$BP_LABEL.elf"
  [ "$(tr -d '\n' < "$T/$BP_LABEL.result")" = 70 ] || { echo "OMGRFN5 composite: $BP_LABEL result drift" >&2; exit 1; }
  observe "$BP_LABEL-pack" 0 20 /dev/null "$T/$BP_LABEL.rfn" no python3 -B "$PACKER" "$T/$BP_LABEL.omgc" "$T/$BP_LABEL.witness" "$T/$BP_LABEL.ckir4" "$T/$BP_LABEL.elf" --result 70
}
build_product exact SourceUnit bootstrap_runtime_record_probe "$SOURCE" "$FIXTURES/source-unit-harness.omg"
build_product authored RuntimePairProbe run "$FIXTURES/authored-declaration-order.omg"
for COMPONENT in omgc witness ckir4 elf; do
  cmp -s "$T/exact.$COMPONENT" "$T/authored.$COMPONENT" && {
    echo "OMGRFN5 composite: $COMPONENT cross-pair control is not distinct" >&2
    exit 1
  }
done
python3 - "$T/exact.rfn" "$T/exact.sha256" <<'PY'
from pathlib import Path
import hashlib,sys
raw=Path(sys.argv[1]).read_bytes(); Path(sys.argv[2]).write_text(hashlib.sha256(raw).hexdigest()+"\n",encoding="ascii")
PY

check() { # checker route expected input case
  CK_NAME=$1 CK_ROUTE=$2 CK_EXPECTED=$3 CK_INPUT=$4 CK_CASE=$5
  observe "check-$CK_NAME-$CK_ROUTE-$CK_CASE" "$CK_EXPECTED" 90 "$CK_INPUT" "$T/$CK_NAME-$CK_ROUTE-$CK_CASE.out" yes "$T/$CK_NAME.$CK_ROUTE"
}
for NAME in $CHECKERS; do check "$NAME" native 0 "$T/exact.rfn" exact; check "$NAME" self 0 "$T/exact.rfn" exact; done

pack_cross() { # label comp witness ckir elf result
  PC_LABEL=$1
  observe "$PC_LABEL-pack" 0 20 /dev/null "$T/$PC_LABEL.rfn" no python3 -B "$PACKER" "$2" "$3" "$4" "$5" --result "$6"
}
pack_cross source-witness "$T/exact.omgc" "$T/authored.witness" "$T/exact.ckir4" "$T/exact.elf" 70
check r1 native 0 "$T/source-witness.rfn" source-witness-opaque
for NAME in r2 r3 r4-lowering r4-source-lowering r4-source-result; do check "$NAME" native 251 "$T/source-witness.rfn" source-witness; done
for NAME in r5-result r5-elf; do check "$NAME" native 0 "$T/source-witness.rfn" source-witness-opaque; done

pack_cross witness-ckir "$T/exact.omgc" "$T/exact.witness" "$T/authored.ckir4" "$T/authored.elf" 70
for NAME in r1 r2 r4-source-lowering r4-source-result r5-result r5-elf; do check "$NAME" native 0 "$T/witness-ckir.rfn" witness-ckir-opaque; done
for NAME in r3 r4-lowering; do check "$NAME" native 251 "$T/witness-ckir.rfn" witness-ckir; done

pack_cross ckir-elf "$T/exact.omgc" "$T/exact.witness" "$T/exact.ckir4" "$T/authored.elf" 70
for NAME in r1 r2 r3 r4-lowering r4-source-lowering r4-source-result r5-result; do check "$NAME" native 0 "$T/ckir-elf.rfn" ckir-elf-opaque; done
check r5-elf native 251 "$T/ckir-elf.rfn" ckir-elf

pack_cross result-pair "$T/exact.omgc" "$T/exact.witness" "$T/exact.ckir4" "$T/exact.elf" 326
for NAME in r1 r2 r3 r4-lowering r4-source-lowering r5-elf; do check "$NAME" native 0 "$T/result-pair.rfn" result-full-opaque; done
for NAME in r4-source-result r5-result; do check "$NAME" native 251 "$T/result-pair.rfn" result-full; done

# Whole-component opacity is checked only at owners that promise not to read it.
printf opaque-front > "$T/opaque.omgc"
printf opaque-witness > "$T/opaque.witness"
printf opaque-ckir4 > "$T/opaque.ckir4"
printf opaque-elf > "$T/opaque.elf"
pack_cross r1-opaque "$T/exact.omgc" "$T/opaque.witness" "$T/opaque.ckir4" "$T/opaque.elf" 71
check r1 native 0 "$T/r1-opaque.rfn" later-components
pack_cross r2-opaque "$T/exact.omgc" "$T/exact.witness" "$T/opaque.ckir4" "$T/opaque.elf" 71
check r2 native 0 "$T/r2-opaque.rfn" artifact-result
pack_cross r3-opaque "$T/opaque.omgc" "$T/exact.witness" "$T/exact.ckir4" "$T/opaque.elf" 71
check r3 native 0 "$T/r3-opaque.rfn" source-elf-result
pack_cross r4-lowering-opaque "$T/exact.omgc" "$T/exact.witness" "$T/exact.ckir4" "$T/opaque.elf" 71
check r4-lowering native 0 "$T/r4-lowering-opaque.rfn" elf-result
pack_cross r4-source-opaque "$T/exact.omgc" "$T/exact.witness" "$T/opaque.ckir4" "$T/opaque.elf" 70
check r4-source-lowering native 0 "$T/r4-source-opaque.rfn" artifacts
check r4-source-result native 0 "$T/r4-source-opaque.rfn" artifacts
pack_cross r5-result-opaque "$T/opaque.omgc" "$T/opaque.witness" "$T/exact.ckir4" "$T/opaque.elf" 70
check r5-result native 0 "$T/r5-result-opaque.rfn" front-elf
pack_cross r5-elf-opaque "$T/opaque.omgc" "$T/opaque.witness" "$T/exact.ckir4" "$T/exact.elf" 70
check r5-elf native 0 "$T/r5-elf-opaque.rfn" front

# Byte-local CKIR/ELF mutations and a common whole-frame resource tooth.
python3 - "$T/exact.rfn" "$T/ckir-opcode.rfn" "$T/elf-byte.rfn" "$T/frame-over.rfn" "$T/version4.rfn" <<'PY'
from pathlib import Path
import struct,sys
raw=Path(sys.argv[1]).read_bytes(); magic,version,flags,cn,wn,kn,en,result,projection=struct.unpack_from('<8s8I',raw)
assert (magic,version,flags,result,projection)==(b'OMGRFN5\0',5,1,70,70)
ck=40+cn+wn; elf=ck+kn
changed=bytearray(raw); counts=struct.unpack_from('<14I',changed,ck+24); at=ck+80
for count,size in zip(counts[:7],(24,20,16,36,20,32,20)): at+=count*size
at+=counts[12]*24+counts[13]*4
site=next(at+i*40+12 for i in range(counts[7]) if changed[at+i*40+12]==13)
changed[site]=12; Path(sys.argv[2]).write_bytes(changed)
changed=bytearray(raw); changed[elf+en-1]^=1; Path(sys.argv[3]).write_bytes(changed)
Path(sys.argv[4]).write_bytes(raw+b'\0'*(4_497_545-len(raw)))
changed=bytearray(raw); changed[:8]=b'OMGRFN4\0'; struct.pack_into('<I',changed,8,4); Path(sys.argv[5]).write_bytes(changed)
PY
for NAME in r1 r2 r3 r4-source-lowering r4-source-result; do check "$NAME" native 0 "$T/ckir-opcode.rfn" ckir-opcode-opaque; done
for NAME in r4-lowering r5-result r5-elf; do check "$NAME" native 251 "$T/ckir-opcode.rfn" ckir-opcode; done
for NAME in r1 r2 r3 r4-lowering r4-source-lowering r4-source-result r5-result; do check "$NAME" native 0 "$T/elf-byte.rfn" elf-byte-opaque; done
check r5-elf native 251 "$T/elf-byte.rfn" elf-byte
for NAME in $CHECKERS; do check "$NAME" native 252 "$T/frame-over.rfn" frame-over; done
for NAME in $CHECKERS; do check "$NAME" native 251 "$T/version4.rfn" version-separation; done

python3 - "$T/exact.rfn" "$T/exact.sha256" <<'PY'
from pathlib import Path
import hashlib,sys
expected=Path(sys.argv[2]).read_text(encoding="ascii").strip(); actual=hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest()
if actual!=expected: raise SystemExit("OMGRFN5 composite: immutable exact carrier changed")
PY

for NAME in $CHECKERS; do cat "$T/$NAME.resources"; done > "$T/resources.tsv"
python3 - "$T/timings.tsv" "$T/resources.tsv" "$T/exact.rfn" "$T/started" <<'PY'
from collections import defaultdict
from pathlib import Path
import sys,time
names=("r1","r2","r3","r4-lowering","r4-source-lowering","r4-source-result","r5-result","r5-elf")
rows=[]; builds=defaultdict(float); runs=defaultdict(float); producer=0.0
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
 sec,label=line.split("\t",1); sec=float(sec); rows.append((sec,label)); matched=False
 for name in names:
  if label.startswith(f"build-{name}-") or label.startswith(f"assemble-{name}-"): builds[name]+=sec; matched=True; break
  if label.startswith(f"check-{name}-"): runs[name]+=sec; matched=True; break
 if not matched: producer+=sec
resources=[]
for line in Path(sys.argv[2]).read_text(encoding="ascii").splitlines():
 name,procs,locals_,tape=line.split("\t"); resources.append(f"{name}={procs}p/{locals_}l/{tape}b")
slow=sorted(rows,reverse=True)[:4]; wall=time.time()-float(Path(sys.argv[4]).read_text())
print("OMGRFN5 same-frame composite timings: "+" ".join(f"{n}=build{builds[n]:.3f}s/run{runs[n]:.3f}s" for n in names)+f" producer={producer:.3f}s command-sum={sum(s for s,_ in rows):.3f}s wall={wall:.3f}s slowest="+",".join(f"{label}:{sec:.3f}s" for sec,label in slow))
print("OMGRFN5 same-frame composite resources: "+" ".join(resources))
print(f"OMGRFN5 same-frame composite: all five responsibilities accepted one unchanged {Path(sys.argv[3]).stat().st_size}-byte exact SourceUnit+harness carrier; source/witness, witness/CKIR4, CKIR4/ELF, and result cross-pairs; physical opacity; local mutations; native/self; and 0/251/252 passed")
PY
