#!/usr/bin/env sh
# Focused persisted-Beta OMGRFN4 responsibility-4 lowering and interval proof.
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
  *) echo "OMGRFN4 responsibility 4: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN4 responsibility 4: skipped ($TOOL absent)"; exit 0; }
done

R="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT"
ENVELOPE="$R/omgrfn4-component-envelope.beta"
BASE_MODEL="$R/omgrfn2-resolved-body-model.beta"
MODEL="$R/omgrfn4-source-body-model.beta"
COMMON="$R/ckir-refinement-source-lowering.beta"
V3="$R/omgrfn3-resolved-body-lowering.beta"
LOWERING="$R/omgrfn4-resolved-body-lowering.beta"
OPERATIONS="$R/omgrfn4-operation-lowering.beta"
ROOTS="$R/omgrfn4-constant-root-correspondence.beta"
INTERVALS="$R/omgrfn4-interval-fixed-point.beta"
PACKER="$R/omgrfn4_bundle.py"
BUILDER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py"
LOW_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-frame.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir3.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v3-to-elf.alp"
FIXTURES="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir3-constant-aggregates"
UNICODE="$OMEGA_REPO_ROOT/compiler/psi/generated/unicode_tables.omg"
for REQUIRED in "$ENVELOPE" "$BASE_MODEL" "$MODEL" "$COMMON" "$V3" "$LOWERING" \
  "$OPERATIONS" "$ROOTS" "$INTERVALS" "$PACKER" "$BUILDER" "$LOW_FRAME" "$RESOLVER" "$LOWERER" "$BACKEND" \
  "$FIXTURES/unicode-harness.omg" "$FIXTURES/renamed-reordered-nested.omg" "$UNICODE"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN4 responsibility 4: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
if [ "${OMEGA_KEEP_R4_TMP:-0}" = 1 ]; then
  echo "OMGRFN4 responsibility 4: keeping $T"
else
  trap 'rm -rf "$T"' EXIT
fi
STARTED=$(date +%s)

filter_procs() { # input output comma-separated exclusions
  python3 -B - "$1" "$2" "$3" <<'PY'
from pathlib import Path
import re, sys
source=Path(sys.argv[1]).read_text(encoding="ascii")
excluded=set(filter(None,sys.argv[3].split(",")))
starts=list(re.finditer(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*\{",source))
pieces=[]
for match in starts:
    depth=1; cursor=match.end()
    while depth and cursor<len(source):
        depth+=(source[cursor]=="{")-(source[cursor]=="}"); cursor+=1
    if depth: raise SystemExit(f"unterminated procedure {match.group(1)}")
    if match.group(1) not in excluded:
        pieces.append(source[match.start():cursor].rstrip()+"\n")
Path(sys.argv[2]).write_text("\n".join(pieces),encoding="ascii")
PY
}

extract_proc() { # input procedure output
  python3 -B - "$1" "$2" "$3" <<'PY'
from pathlib import Path
import re,sys
source=Path(sys.argv[1]).read_text(encoding="ascii"); name=sys.argv[2]
match=re.search(rf"(?m)^proc\s+{re.escape(name)}\s*\([^)]*\)\s*\{{",source)
if not match: raise SystemExit(f"missing procedure {name}")
depth=1; cursor=match.end()
while depth and cursor<len(source):
    depth+=(source[cursor]=="{")-(source[cursor]=="}"); cursor+=1
if depth: raise SystemExit(f"unterminated procedure {name}")
Path(sys.argv[3]).write_text(source[match.start():cursor]+"\n",encoding="ascii")
PY
}

prune_reachable() { # input output root
  python3 -B - "$1" "$2" "$3" <<'PY'
from pathlib import Path
import re,sys
source=Path(sys.argv[1]).read_text(encoding="ascii")
starts=list(re.finditer(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*\{",source))
procedures={}
for match in starts:
    depth=1; cursor=match.end()
    while depth and cursor<len(source):
        depth+=(source[cursor]=="{")-(source[cursor]=="}"); cursor+=1
    if depth: raise SystemExit(f"unterminated procedure {match.group(1)}")
    procedures[match.group(1)]=source[match.start():cursor].rstrip()+"\n"
reachable=set(); pending=[sys.argv[3]]
while pending:
    name=pending.pop()
    if name in reachable: continue
    if name not in procedures: raise SystemExit(f"missing reachable procedure {name}")
    reachable.add(name)
    body=procedures[name]
    for called in re.findall(r"\b([A-Za-z_]\w*)\s*\(",body):
        if called in procedures and called not in reachable: pending.append(called)
Path(sys.argv[2]).write_text("\n".join(procedures[name] for name in procedures if name in reachable),encoding="ascii")
PY
}

sed 's/omgrfn2_component/omgrfn4_component/g' "$BASE_MODEL" > "$T/base-model-all.beta"
filter_procs "$T/base-model-all.beta" "$T/base-model.beta" \
  'l4_model_declarations,l4_model_types_records_fields,l4_model_machines_blocks,l4_model_prepare'
filter_procs "$COMMON" "$T/common.beta" \
  'src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_expression,src_low_transition,src_low_body,src_reconstruct_lowering_check,src_refinement_lowering_check,main'
python3 -B - "$T/common.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="to bad when (src_low_stack(8,2) > 2147483647-src_low_g(22))  "
if text.count(old)!=1:
    raise SystemExit("OMGRFN4 responsibility 4: inherited Add-bound anchor drifted")
p.write_text(text.replace(old,""),encoding="ascii")
PY
sed '/^proc v3_ckir_header_check/,$d' "$V3" > "$T/v3-prefix.beta"
python3 -B - "$T/v3-prefix.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state index_bound { to bad when (index_high >= src_type(base_type,5))  to index_left when (src_low_g(30) >= index_tree)"
new="state index_bound { to index_left when (src_low_g(30) >= index_tree)"
if text.count(old)!=1:
    raise SystemExit("OMGRFN4 responsibility 4: V3 index-bound anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
extract_proc "$COMMON" src_low_transition "$T/guarded.beta"
python3 -B - "$T/guarded.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
text=text.replace("proc src_low_transition()", "proc v4_guarded_transition_after_keyword()", 1)
text=text.replace("state keyword { src_next()  to guard }", "state keyword { to guard }", 1)
p.write_text(text,encoding="ascii")
PY
for PROC in v4_ckir_header_check v4_token_equal_named; do
  extract_proc "$LOWERING" "$PROC" "$T/$PROC.beta"
done

cp "$ENVELOPE" "$T/lowering.beta"
cat "$T/base-model.beta" "$MODEL" "$T/common.beta" "$T/v3-prefix.beta" \
  "$T/guarded.beta" "$T/v4_ckir_header_check.beta" \
  "$T/v4_token_equal_named.beta" "$OPERATIONS" >> "$T/lowering.beta"
prune_reachable "$T/lowering.beta" "$T/lowering-pruned.beta" main
mv "$T/lowering-pruned.beta" "$T/lowering.beta"

PROCEDURES=$(awk '/^proc / { n++ } END { print n+0 }' "$T/lowering.beta")
[ "$PROCEDURES" -le 128 ] || { echo "OMGRFN4 responsibility 4: lowering procedures $PROCEDURES" >&2; exit 1; }
MAX_LOCALS=$(python3 -B - "$T/lowering.beta" <<'PY'
import re,sys
s=open(sys.argv[1],encoding="ascii").read(); maximum=0
for m in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{",s,re.M):
    end=s.find("\nproc ",m.end()); body=s[m.end():end if end>=0 else len(s)]
    maximum=max(maximum,sum(bool(x.strip()) for x in m.group(1).split(","))+len(re.findall(r"\blet\s+[A-Za-z_]\w*",body)))
print(maximum)
PY
)
[ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN4 responsibility 4: lowering locals $MAX_LOCALS" >&2; exit 1; }

BC="$T/bc"; SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"; ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null
"$BC" < "$T/lowering.beta" > "$T/lowering.asm" || {
  echo "OMGRFN4 responsibility 4: Beta compilation failed for $T/lowering.beta" >&2
  exit 1
}
"$ASM" < "$T/lowering.asm" > "$T/lowering.tape"
TAPE_BYTES=$(wc -c < "$T/lowering.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || { echo "OMGRFN4 responsibility 4: lowering tape $TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/lowering.tape" "$SEED" "$T/lowering" >/dev/null 2>&1

# The constant/root checker is a separate bounded executable.  It derives the
# complete source constant DAG and binds each aggregate occurrence to opcode 11;
# the operation checker intentionally treats only that one immediate as opaque.
extract_proc "$COMMON" ckir_u32 "$T/ckir_u32.beta"
for PROC in v4_ckir_constant v4_ckir_child v4_raw v4_raw_set v4_raw_child \
  v4_raw_child_set v4_ckir_header_check v4_token_equal_named v4_constant_match \
  v4_constant_add v4_record_field v4_parse_constant v4_constants_complete; do
  extract_proc "$LOWERING" "$PROC" "$T/root-$PROC.beta"
done
cp "$ENVELOPE" "$T/roots.beta"
cat "$T/base-model.beta" "$MODEL" "$T/ckir_u32.beta" \
  "$T/root-v4_ckir_constant.beta" "$T/root-v4_ckir_child.beta" \
  "$T/root-v4_raw.beta" "$T/root-v4_raw_set.beta" \
  "$T/root-v4_raw_child.beta" "$T/root-v4_raw_child_set.beta" \
  "$T/root-v4_ckir_header_check.beta" "$T/root-v4_token_equal_named.beta" \
  "$T/root-v4_constant_match.beta" "$T/root-v4_constant_add.beta" \
  "$T/root-v4_record_field.beta" "$T/root-v4_parse_constant.beta" \
  "$T/root-v4_constants_complete.beta" "$ROOTS" >> "$T/roots.beta"
prune_reachable "$T/roots.beta" "$T/roots-pruned.beta" main
mv "$T/roots-pruned.beta" "$T/roots.beta"
ROOT_PROCEDURES=$(awk '/^proc / { n++ } END { print n+0 }' "$T/roots.beta")
[ "$ROOT_PROCEDURES" -le 128 ] || { echo "OMGRFN4 responsibility 4: root procedures $ROOT_PROCEDURES" >&2; exit 1; }
ROOT_MAX_LOCALS=$(python3 -B - "$T/roots.beta" <<'PY'
import re,sys
s=open(sys.argv[1],encoding="ascii").read(); maximum=0
for m in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{",s,re.M):
    end=s.find("\nproc ",m.end()); body=s[m.end():end if end>=0 else len(s)]
    maximum=max(maximum,sum(bool(x.strip()) for x in m.group(1).split(","))+len(re.findall(r"\blet\s+[A-Za-z_]\w*",body)))
print(maximum)
PY
)
[ "$ROOT_MAX_LOCALS" -le 32 ] || { echo "OMGRFN4 responsibility 4: root locals $ROOT_MAX_LOCALS" >&2; exit 1; }
"$BC" < "$T/roots.beta" > "$T/roots.asm" || { echo "OMGRFN4 responsibility 4: Beta compilation failed for root checker" >&2; exit 1; }
"$ASM" < "$T/roots.asm" > "$T/roots.tape"
ROOT_TAPE_BYTES=$(wc -c < "$T/roots.tape" | tr -d ' ')
[ "$ROOT_TAPE_BYTES" -le 262140 ] || { echo "OMGRFN4 responsibility 4: root tape $ROOT_TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/roots.tape" "$SEED" "$T/roots" >/dev/null 2>&1

# Fixed-point analyzer: source/witness only, with analysis-pass Add/index
# obligations deferred and then restored for canonical replay.
filter_procs "$COMMON" "$T/interval-common.beta" \
  'src_low_decode_validated_ckir,src_low_primary,src_low_parse_arguments,src_low_transition,src_low_body,src_low_all_bodies,src_reconstruct_lowering_check,src_refinement_lowering_check,main'
python3 -B - "$T/interval-common.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state bounds { to bad when (src_low_g(21) > src_low_g(22))  to bad when (src_low_stack(8,1) > src_low_stack(8,2))  to bad when (src_low_stack(8,2) > 2147483647-src_low_g(22))  to literal_emit when (target == 4294967295)  to typed_emit }"
new="state bounds { to bad when (src_low_g(21) > src_low_g(22))  to bad when (src_low_stack(8,1) > src_low_stack(8,2))  to bound_ready when (word[23400008] == 1)  to bad when (src_low_stack(8,2) > 2147483647-src_low_g(22))  to bound_ready }\n    state bound_ready { to literal_emit when (target == 4294967295)  to typed_emit }"
if text.count(old)!=1: raise SystemExit("interval Add anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
extract_proc "$COMMON" src_low_primary "$T/interval-primary.beta"
python3 -B - "$T/interval-primary.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state parameter_refine { to parameter_advance when (src_low_g(5) == 4294967295)  to parameter_advance when (word[19600000+src_low_g(5)*32] != 1)  to parameter_advance when (word[19600008+src_low_g(5)*32] != 2)  to parameter_advance when (word[19600016+src_low_g(5)*32] != id)  to parameter_advance when (src_low_g(22) <= word[19600024+src_low_g(5)*32])  src_low_gset(22,word[19600024+src_low_g(5)*32])  to parameter_advance }"
new="""state parameter_refine { to parameter_fact when (src_low_g(5) == 4294967295)  to parameter_fact when (word[23000000+src_low_g(5)*8] == 0)  to parameter_fact when (word[23100000+id*8] == 0)  src_low_gset(21,word[23200000+id*8])  src_low_gset(22,word[23300000+id*8])  to parameter_fact }
    state parameter_fact { to arm_fact when (src_low_g(5) == 4294967295)  to arm_fact when (word[19600000+src_low_g(5)*32] != 1)  to arm_fact when (word[19600008+src_low_g(5)*32] != 2)  to arm_fact when (word[19600016+src_low_g(5)*32] != id)  to arm_fact when (src_low_g(22) <= word[19600024+src_low_g(5)*32])  src_low_gset(22,word[19600024+src_low_g(5)*32])  to arm_fact }
    state arm_fact { to parameter_advance when (word[23400016] != 1)  to parameter_advance when (word[23400024] != 1)  to parameter_advance when (word[23400032] != 2)  to parameter_advance when (word[23400040] != id)  to parameter_advance when (src_low_g(22) <= word[23400048])  src_low_gset(22,word[23400048])  to parameter_advance }"""
if text.count(old)!=1: raise SystemExit("interval primary anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
sed '/^proc v3_ckir_header_check/,$d' "$V3" > "$T/interval-v3.beta"
python3 -B - "$T/interval-v3.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state index_bound { to bad when (index_high >= src_type(base_type,5))  to index_left when (src_low_g(30) >= index_tree)"
new="state index_bound { to index_left when (word[23400008] == 1)  to bad when (index_high >= src_type(base_type,5))  to index_left when (src_low_g(30) >= index_tree)"
if text.count(old)!=1: raise SystemExit("interval index anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
cp "$T/guarded.beta" "$T/interval-guarded.beta"
python3 -B - "$T/interval-guarded.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
text=text.replace("fact_hi=src_low_g(44)  to done", "fact_hi=src_low_g(44)  word[23400024]=fact_valid  word[23400032]=fact_kind  word[23400040]=fact_id  word[23400048]=fact_hi  word[23400056]=src_low_g(45)  to done",1)
text=text.replace("true_seen=1  arm_kind=1", "true_seen=1  arm_kind=1  word[23400016]=1",1)
text=text.replace("false_seen=1  arm_kind=2", "false_seen=1  arm_kind=2  word[23400016]=2",1)
text=text.replace("wild_seen=1  arm_kind=3", "wild_seen=1  arm_kind=3  word[23400016]=3",1)
p.write_text(text,encoding="ascii")
PY
for PROC in v4_skip_constant src_low_expression src_low_body; do extract_proc "$OPERATIONS" "$PROC" "$T/interval-$PROC.beta"; done
python3 -B - "$T/interval-src_low_body.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="src_low_gset(52,ckir_row_word(7,src_low_g(0),40,32))"
if text.count(old)!=1: raise SystemExit("interval aggregate anchor drifted")
p.write_text(text.replace(old,"src_low_gset(52,0)"),encoding="ascii")
PY
cp "$ENVELOPE" "$T/interval.beta"
cat "$T/base-model.beta" "$MODEL" "$T/interval-common.beta" "$T/interval-primary.beta" \
  "$T/interval-v3.beta" "$T/interval-guarded.beta" "$T/v4_token_equal_named.beta" \
  "$T/interval-v4_skip_constant.beta" "$T/interval-src_low_expression.beta" \
  "$T/interval-src_low_body.beta" "$INTERVALS" >> "$T/interval.beta"
prune_reachable "$T/interval.beta" "$T/interval-pruned.beta" main
mv "$T/interval-pruned.beta" "$T/interval.beta"
INTERVAL_PROCEDURES=$(awk '/^proc / { n++ } END { print n+0 }' "$T/interval.beta")
[ "$INTERVAL_PROCEDURES" -le 128 ] || { echo "OMGRFN4 responsibility 4: interval procedures $INTERVAL_PROCEDURES" >&2; exit 1; }
INTERVAL_MAX_LOCALS=$(python3 -B - "$T/interval.beta" <<'PY'
import re,sys
s=open(sys.argv[1],encoding="ascii").read(); maximum=0
for m in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{",s,re.M):
    end=s.find("\nproc ",m.end()); body=s[m.end():end if end>=0 else len(s)]
    maximum=max(maximum,sum(bool(x.strip()) for x in m.group(1).split(","))+len(re.findall(r"\blet\s+[A-Za-z_]\w*",body)))
print(maximum)
PY
)
[ "$INTERVAL_MAX_LOCALS" -le 32 ] || { echo "OMGRFN4 responsibility 4: interval locals $INTERVAL_MAX_LOCALS" >&2; exit 1; }
"$BC" < "$T/interval.beta" > "$T/interval.asm" || { echo "OMGRFN4 responsibility 4: Beta compilation failed for interval checker" >&2; exit 1; }
"$ASM" < "$T/interval.asm" > "$T/interval.tape"
INTERVAL_TAPE_BYTES=$(wc -c < "$T/interval.tape" | tr -d ' ')
[ "$INTERVAL_TAPE_BYTES" -le 262140 ] || { echo "OMGRFN4 responsibility 4: interval tape $INTERVAL_TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/interval.tape" "$SEED" "$T/interval" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend" >/dev/null

run_expect() { # exe input expected output label
  set +e; "$1" < "$2" > "$4" 2> "$4.stderr"; ACTUAL=$?; set -e
  [ "$ACTUAL" -eq "$3" ] || { echo "OMGRFN4 responsibility 4: $5 returned $ACTUAL expected $3" >&2; sed -n '1,20p' "$4.stderr" >&2; exit 1; }
  [ "$3" -eq 0 ] || [ ! -s "$4" ] || { echo "OMGRFN4 responsibility 4: $5 published output" >&2; exit 1; }
}

build_case() { # name owner machine source...
  NAME=$1 OWNER=$2 MACHINE=$3; shift 3
  python3 -B "$BUILDER" build "$T/$NAME.omgc" "$OWNER" "$MACHINE" "$@"
  run_expect "$T/resolver" "$T/$NAME.omgc" 0 "$T/$NAME.witness" "$NAME resolver"
  python3 -B "$LOW_FRAME" pack "$T/$NAME.omgc" "$T/$NAME.witness" > "$T/$NAME.low"
  run_expect "$T/lowerer" "$T/$NAME.low" 0 "$T/$NAME.ckir" "$NAME lowerer"
  run_expect "$T/backend" "$T/$NAME.ckir" 0 "$T/$NAME.elf" "$NAME backend"
  python3 -B "$PACKER" "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.ckir" "$T/$NAME.elf" --result 70 > "$T/$NAME.rfn"
}

build_case compact AggregateProbe run "$FIXTURES/renamed-reordered-nested.omg"
run_expect "$T/lowering" "$T/compact.rfn" 0 "$T/compact.out" "renamed/reordered/nested"
run_expect "$T/roots" "$T/compact.rfn" 0 "$T/compact.roots.out" "renamed/reordered/nested roots"
run_expect "$T/interval" "$T/compact.rfn" 0 "$T/compact.interval.out" "renamed/reordered interval replay"

python3 -B - "$FIXTURES/cyclic-range-custody.omg" "$T/cyclic-reordered.omg" \
  "$FIXTURES/arm-local-edge-argument.omg" "$T/arm-no-fact.omg" \
  "$FIXTURES/negative-missing-cycle-predecessor.omg" "$T/missing-reordered.omg" \
  "$T/impossible-true.omg" <<'PY'
from pathlib import Path
import sys
cyclic=Path(sys.argv[1]).read_text(encoding="ascii")
def state_span(text,name):
    start=text.index(f"    state {name}(")
    brace=text.index("{",start); depth=1; pos=brace+1
    while depth:
        depth+=(text[pos]=="{")-(text[pos]=="}"); pos+=1
    while pos<len(text) and text[pos]=="\n": pos+=1
    return start,pos
names=("narrow","indexed","forward","final","pass","fail")
spans={name:state_span(cyclic,name) for name in names}
prefix=cyclic[:spans["narrow"][0]]; suffix=cyclic[spans["fail"][1]:]
parts={name:cyclic[a:b] for name,(a,b) in spans.items()}
Path(sys.argv[2]).write_text(prefix+parts["forward"]+parts["indexed"]+parts["narrow"]+parts["final"]+parts["pass"]+parts["fail"]+suffix,encoding="ascii")
arm=Path(sys.argv[3]).read_text(encoding="ascii")
old="            true -> accept(self.values[index])\n            _ -> fail()"
new="            true -> accept(70)\n            _ -> accept(self.values[index])"
if arm.count(old)!=1: raise SystemExit("arm-local mutation anchor drifted")
Path(sys.argv[4]).write_text(arm.replace(old,new),encoding="ascii")
missing=Path(sys.argv[5]).read_text(encoding="ascii")
a=state_span(missing,"inspect"); b=state_span(missing,"back")
Path(sys.argv[6]).write_text(missing[:a[0]]+missing[b[0]:b[1]]+missing[a[0]:a[1]]+missing[b[1]:],encoding="ascii")
Path(sys.argv[7]).write_text(r'''// `< 0` has an impossible true arm; the checker also directly asserts that
// neither target reachability nor interval transfer changes for this arm.
data ImpossibleArmProbe {}

machine ImpossibleArmProbe::run(&mut self) -> u8 {
    transition { _ -> start(0) }

    state start(&mut self, index: u32 in Trapping) {
        transition index < 0 {
            true -> fail()
            _ -> pass()
        }
    }

    state pass(&mut self) { 70 }
    state fail(&mut self) { 71 }
}
''',encoding="ascii")
PY

build_case cyclic CustodyCycle run "$FIXTURES/cyclic-range-custody.omg"
run_expect "$T/interval" "$T/cyclic.rfn" 0 "$T/cyclic.interval.out" "cyclic interval fixed point"
build_case cyclic_reordered CustodyCycle run "$T/cyclic-reordered.omg"
run_expect "$T/interval" "$T/cyclic_reordered.rfn" 0 "$T/cyclic-reordered.interval.out" "cyclic declaration-order independence"
build_case impossible_true ImpossibleArmProbe run "$T/impossible-true.omg"
run_expect "$T/interval" "$T/impossible_true.rfn" 0 "$T/impossible-true.interval.out" "less-than-zero impossible true arm"

build_interval_negative() { # name owner source
  NAME=$1 OWNER=$2 SOURCE=$3
  python3 -B "$BUILDER" build "$T/$NAME.omgc" "$OWNER" run "$SOURCE"
  run_expect "$T/resolver" "$T/$NAME.omgc" 0 "$T/$NAME.witness" "$NAME resolver"
  python3 -B "$LOW_FRAME" pack "$T/$NAME.omgc" "$T/$NAME.witness" > "$T/$NAME.low"
  run_expect "$T/lowerer" "$T/$NAME.low" 251 "$T/$NAME.native.out" "$NAME producer interval rejection"
  python3 -B "$PACKER" "$T/$NAME.omgc" "$T/$NAME.witness" "$T/cyclic.ckir" "$T/cyclic.elf" --result 70 > "$T/$NAME.rfn"
  run_expect "$T/interval" "$T/$NAME.rfn" 251 "$T/$NAME.interval.out" "$NAME interval rejection"
}
build_interval_negative arm_no_fact ArmArgumentProbe "$T/arm-no-fact.omg"
build_interval_negative missing_cycle MissingCycleProbe "$FIXTURES/negative-missing-cycle-predecessor.omg"
build_interval_negative missing_reordered MissingCycleProbe "$T/missing-reordered.omg"

build_case unicode UnicodeTables bootstrap_constant_aggregate_probe "$UNICODE" "$FIXTURES/unicode-harness.omg"
run_expect "$T/lowering" "$T/unicode.rfn" 0 "$T/unicode.out" "exact Unicode+harness"
run_expect "$T/roots" "$T/unicode.rfn" 0 "$T/unicode.roots.out" "exact Unicode+harness roots"
run_expect "$T/interval" "$T/unicode.rfn" 0 "$T/unicode.interval.out" "exact Unicode+harness interval fixed point"

python3 -B - "$T/compact.rfn" "$T/unicode.rfn" "$T/root-mutated.rfn" \
  "$T/op12-mutated.rfn" "$T/oversized.rfn" "$T/child-span-mutated.rfn" \
  "$T/block-partition-mutated.rfn" <<'PY'
from pathlib import Path
import struct,sys
frame=bytearray(Path(sys.argv[1]).read_bytes())
omg,witness,ckir=struct.unpack_from("<3I",frame,16)
base=40+omg+witness
counts=struct.unpack_from("<14I",frame,base+24)
cursor=base+80
for count,size in zip(counts[:7],(24,20,16,36,20,32,20)):
    cursor+=count*size
constants=cursor; cursor+=counts[12]*24+counts[13]*4
operations=cursor
root=bytearray(frame)
for op in range(counts[7]):
    at=operations+op*40
    if root[at+12]==11:
        current=struct.unpack_from("<I",root,at+32)[0]
        struct.pack_into("<I",root,at+32,(current-1) & 0xffffffff)
        break
else: raise SystemExit("compact control has no opcode 11")
Path(sys.argv[3]).write_bytes(root)
op12=bytearray(frame)
for op in range(counts[7]):
    at=operations+op*40
    if op12[at+12]==12:
        op12[at+12]=9
        break
else: raise SystemExit("compact control has no opcode 12")
Path(sys.argv[4]).write_bytes(op12)
Path(sys.argv[5]).write_bytes(frame+b"\0"*(4_497_545-len(frame)))
child_span=bytearray(frame)
for node in range(counts[12]):
    at=constants+node*24
    if struct.unpack_from("<I",child_span,at+12)[0]>0:
        struct.pack_into("<I",child_span,at+8,counts[13])
        break
else: raise SystemExit("compact control has no structural constant")
Path(sys.argv[6]).write_bytes(child_span)

uframe=bytearray(Path(sys.argv[2]).read_bytes())
uomg,uwitness=struct.unpack_from("<2I",uframe,16)
wbase=40+uomg
wcounts=struct.unpack_from("<11I",uframe,wbase+20)
wcursor=wbase+72
for count,size in zip(wcounts[:7],(36,48,28,28,24,24,24)):
    wcursor+=count*size
if wcounts[7]<2: raise SystemExit("Unicode control lacks a second machine")
struct.pack_into("<I",uframe,wcursor+40+28,0)
Path(sys.argv[7]).write_bytes(uframe)
PY
run_expect "$T/lowering" "$T/root-mutated.rfn" 0 "$T/root-mutated.ops.out" "root mutation operation opacity"
run_expect "$T/roots" "$T/root-mutated.rfn" 251 "$T/root-mutated.roots.out" "source-derived opcode-11 root mutation"
run_expect "$T/interval" "$T/root-mutated.rfn" 0 "$T/root-mutated.interval.out" "root mutation interval opacity"
run_expect "$T/lowering" "$T/op12-mutated.rfn" 251 "$T/op12-mutated.ops.out" "opcode-12 mutation"
run_expect "$T/interval" "$T/op12-mutated.rfn" 0 "$T/op12-mutated.interval.out" "opcode-12 mutation interval opacity"
run_expect "$T/roots" "$T/child-span-mutated.rfn" 251 "$T/child-span-mutated.out" "constant child-span safety"
run_expect "$T/roots" "$T/block-partition-mutated.rfn" 251 "$T/block-partition-mutated.out" "machine block-partition safety"
run_expect "$T/lowering" "$T/oversized.rfn" 252 "$T/oversized.ops.out" "operation public ceiling"
run_expect "$T/roots" "$T/oversized.rfn" 252 "$T/oversized.roots.out" "root public ceiling"
run_expect "$T/interval" "$T/oversized.rfn" 252 "$T/oversized.interval.out" "interval public ceiling"

ELAPSED=$(($(date +%s)-STARTED))
echo "OMGRFN4 responsibility 4: source-derived constants/roots, CKIR3 bodies, and cyclic interval fixed-point replay passed (${ELAPSED}s; ops ${PROCEDURES}/128 procedures, ${MAX_LOCALS}/32 locals, ${TAPE_BYTES}/262140 tape bytes; roots ${ROOT_PROCEDURES}/128 procedures, ${ROOT_MAX_LOCALS}/32 locals, ${ROOT_TAPE_BYTES}/262140 tape bytes; intervals ${INTERVAL_PROCEDURES}/128 procedures, ${INTERVAL_MAX_LOCALS}/32 locals, ${INTERVAL_TAPE_BYTES}/262140 tape bytes)"
