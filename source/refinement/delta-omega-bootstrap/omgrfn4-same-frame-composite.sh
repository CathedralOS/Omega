#!/usr/bin/env sh
# Same-exact-frame composition of all five independent OMGRFN4 duties.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
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
  *) echo "OMGRFN4 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN4 same-frame composite: skipped ($TOOL absent)"
    exit 0
  }
done

R="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT"
ENVELOPE="$R/omgrfn4-component-envelope.beta"
L1="$R/omgrfn4-frame-omgcomp-custody.beta"
L2="$R/omgrfn4-source-witness-independent.beta"
L3="$R/omgrfn4-witness-ckir3-tables.beta"
L4BASE="$R/omgrfn2-resolved-body-model.beta"
L4MODEL="$R/omgrfn4-source-body-model.beta"
L4COMMON="$R/ckir-refinement-source-lowering.beta"
L4V3="$R/omgrfn3-resolved-body-lowering.beta"
L4LOW="$R/omgrfn4-resolved-body-lowering.beta"
L4OPERATIONS="$R/omgrfn4-operation-lowering.beta"
L4ROOTS="$R/omgrfn4-constant-root-correspondence.beta"
L4INTERVAL="$R/omgrfn4-interval-fixed-point.beta"
L4RESULT="$R/omgrfn4-source-only-result.beta"
L5ARTIFACT="$R/ckir3-refinement-artifact.beta"
L5RESULT="$R/ckir3-refinement-result.beta"
L5ELF="$R/ckir3-refinement-elf.beta"
PACKER="$R/omgrfn4_bundle.py"
BUILDER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py"
LOW_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-frame.py"
IR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v3_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v3_reference.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir3.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v3-to-elf.alp"
FIXTURES="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir3-constant-aggregates"
UNICODE="$OMEGA_REPO_ROOT/source/psi/generated/unicode_tables.omg"
HARNESS="$FIXTURES/unicode-harness.omg"
GENERATED_CUSTODY="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/generated_source_custody.py"
GENERATED_RECIPE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/generated-source-custody/unicode-tables.recipe.json"
for REQUIRED in "$ENVELOPE" "$L1" "$L2" "$L3" "$L4BASE" "$L4MODEL" \
  "$L4COMMON" "$L4V3" "$L4LOW" "$L4OPERATIONS" "$L4ROOTS" \
  "$L4INTERVAL" "$L4RESULT" \
  "$L5ARTIFACT" "$L5RESULT" "$L5ELF" "$PACKER" "$BUILDER" "$LOW_FRAME" \
  "$IR_REFERENCE" "$ELF_REFERENCE" "$RESOLVER" "$LOWERER" "$BACKEND" \
  "$UNICODE" "$HARNESS" "$GENERATED_CUSTODY" "$GENERATED_RECIPE"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN4 same-frame composite: missing $REQUIRED" >&2
    exit 1
  }
done

# OMGRFN4 continues to reconstruct the canonical committed source frame. This
# preflight establishes that the generated extent in that frame is exactly and
# deterministically reproduced by its sealed recipe.
python3 -B "$GENERATED_CUSTODY" reproduce "$GENERATED_RECIPE"

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_OMGRFN4_COMPOSITE_TEMP:-0}" = 1 ]; then
    echo "OMGRFN4 same-frame composite: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT
: > "$T/timings.tsv"
: > "$T/check-queue.tsv"
python3 - "$T/started" <<'PY'
from pathlib import Path
import sys, time
Path(sys.argv[1]).write_text(f"{time.time():.9f}\n", encoding="ascii")
PY

# All subprocesses receive a process-group timeout.  The runner records every
# build and observation without adding viewer/debug-output work to the gate.
python3 - "$T/run.py" <<'PY'
from pathlib import Path
import os, signal, subprocess, sys, time

Path(sys.argv[1]).write_text(r'''#!/usr/bin/env python3
from pathlib import Path
import os, signal, subprocess, sys, time

label, expected, timeout, source, output, empty, timings, *command = sys.argv[1:]
started = time.monotonic()
with open(source, "rb") as input_file:
    process = subprocess.Popen(
        command, stdin=input_file, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(timeout=float(timeout))
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        Path(output).write_bytes(stdout)
        Path(output + ".stderr").write_bytes(stderr)
        raise SystemExit(f"{label} exceeded {timeout}s")
elapsed = time.monotonic() - started
Path(output).write_bytes(stdout)
Path(output + ".stderr").write_bytes(stderr)
with open(timings, "a", encoding="ascii") as report:
    report.write(f"{elapsed:.6f}\t{label}\n")
if process.returncode != int(expected):
    if stderr:
        sys.stderr.buffer.write(stderr[-4096:])
    raise SystemExit(f"{label} returned {process.returncode}, expected {expected}")
if empty == "yes" and stdout:
    raise SystemExit(f"{label} published {len(stdout)} stdout bytes")
''', encoding="ascii")
PY

observe() { # label status timeout stdin stdout require-empty command...
  OBS_LABEL=$1 OBS_STATUS=$2 OBS_TIMEOUT=$3 OBS_INPUT=$4 OBS_OUTPUT=$5 OBS_EMPTY=$6
  shift 6
  python3 "$T/run.py" "$OBS_LABEL" "$OBS_STATUS" "$OBS_TIMEOUT" \
    "$OBS_INPUT" "$OBS_OUTPUT" "$OBS_EMPTY" "$T/timings.tsv" "$@"
}

filter_procs() { # input output comma-separated exclusions
  python3 -B - "$1" "$2" "$3" <<'PY'
from pathlib import Path
import re, sys
source=Path(sys.argv[1]).read_text(encoding="ascii")
excluded=set(filter(None,sys.argv[3].split(",")))
pieces=[]
for match in re.finditer(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*\{",source):
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
import re, sys
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
import re, sys
source=Path(sys.argv[1]).read_text(encoding="ascii")
procedures={}
for match in re.finditer(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*\{",source):
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
    for called in re.findall(r"\b([A-Za-z_]\w*)\s*\(",procedures[name]):
        if called in procedures and called not in reachable: pending.append(called)
Path(sys.argv[2]).write_text(
    "\n".join(procedures[name] for name in procedures if name in reachable),
    encoding="ascii",
)
PY
}

# Build exactly one persisted program for each independently observable claim.
cp "$L1" "$T/layer1.beta"
printf '\nproc main() { return omgrfn4_layer1_check() }\n' >> "$T/layer1.beta"
cp "$L2" "$T/layer2.beta"
printf '\nproc main() { return omgrfn4_r2_check() }\n' >> "$T/layer2.beta"
cp "$L3" "$T/layer3.beta"

# Responsibility 4 uses two independently bounded CKIR-correspondence
# executables. Reproduce the focused gate's frozen compositions exactly: the
# operation checker owns every executable row except the source-derived op-11
# root, while the root checker owns the complete aggregate graph and that join.
sed 's/omgrfn2_component/omgrfn4_component/g' "$L4BASE" > "$T/l4-base-all.beta"
filter_procs "$T/l4-base-all.beta" "$T/l4-base.beta" \
  'l4_model_declarations,l4_model_types_records_fields,l4_model_machines_blocks,l4_model_prepare'
filter_procs "$L4COMMON" "$T/l4-common.beta" \
  'src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_expression,src_low_transition,src_low_body,src_reconstruct_lowering_check,src_refinement_lowering_check,main'
python3 -B - "$T/l4-common.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="to bad when (src_low_stack(8,2) > 2147483647-src_low_g(22))  "
if text.count(old)!=1:
    raise SystemExit("OMGRFN4 composite: inherited Add-bound anchor drifted")
p.write_text(text.replace(old,""),encoding="ascii")
PY
sed '/^proc v3_ckir_header_check/,$d' "$L4V3" > "$T/l4-v3-prefix.beta"
python3 -B - "$T/l4-v3-prefix.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state index_bound { to bad when (index_high >= src_type(base_type,5))  to index_left when (src_low_g(30) >= index_tree)"
new="state index_bound { to index_left when (src_low_g(30) >= index_tree)"
if text.count(old)!=1:
    raise SystemExit("OMGRFN4 composite: V3 index-bound anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
extract_proc "$L4COMMON" src_low_transition "$T/l4-guarded.beta"
python3 -B - "$T/l4-guarded.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
text=text.replace("proc src_low_transition()", "proc v4_guarded_transition_after_keyword()", 1)
text=text.replace("state keyword { src_next()  to guard }", "state keyword { to guard }", 1)
p.write_text(text,encoding="ascii")
PY
for PROC in v4_ckir_header_check v4_token_equal_named; do
  extract_proc "$L4LOW" "$PROC" "$T/l4-$PROC.beta"
done
cat "$ENVELOPE" "$T/l4-base.beta" "$L4MODEL" "$T/l4-common.beta" \
  "$T/l4-v3-prefix.beta" "$T/l4-guarded.beta" \
  "$T/l4-v4_ckir_header_check.beta" "$T/l4-v4_token_equal_named.beta" \
  "$L4OPERATIONS" > "$T/layer4-operations-all.beta"
prune_reachable "$T/layer4-operations-all.beta" "$T/layer4-operations.beta" main

extract_proc "$L4COMMON" ckir_u32 "$T/l4-ckir_u32.beta"
L4_ROOT_PARTS=''
for PROC in v4_ckir_constant v4_ckir_child v4_raw v4_raw_set v4_raw_child \
  v4_raw_child_set v4_ckir_header_check v4_token_equal_named v4_constant_match \
  v4_constant_add v4_record_field v4_parse_constant v4_constants_complete; do
  extract_proc "$L4LOW" "$PROC" "$T/l4-root-$PROC.beta"
  L4_ROOT_PARTS="$L4_ROOT_PARTS $T/l4-root-$PROC.beta"
done
# shellcheck disable=SC2086 -- each generated procedure is one whitespace-free path.
cat "$ENVELOPE" "$T/l4-base.beta" "$L4MODEL" "$T/l4-ckir_u32.beta" \
  $L4_ROOT_PARTS "$L4ROOTS" > "$T/layer4-roots-all.beta"
prune_reachable "$T/layer4-roots-all.beta" "$T/layer4-roots.beta" main

# The interval executable owns the least fixed point and obligation-restoring
# replay independently of claimed CKIR/ELF/result bytes.
filter_procs "$L4COMMON" "$T/l4-interval-common.beta" \
  'src_low_decode_validated_ckir,src_low_primary,src_low_parse_arguments,src_low_transition,src_low_body,src_low_all_bodies,src_reconstruct_lowering_check,src_refinement_lowering_check,main'
python3 -B - "$T/l4-interval-common.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state bounds { to bad when (src_low_g(21) > src_low_g(22))  to bad when (src_low_stack(8,1) > src_low_stack(8,2))  to bad when (src_low_stack(8,2) > 2147483647-src_low_g(22))  to literal_emit when (target == 4294967295)  to typed_emit }"
new="state bounds { to bad when (src_low_g(21) > src_low_g(22))  to bad when (src_low_stack(8,1) > src_low_stack(8,2))  to bound_ready when (word[23400008] == 1)  to bad when (src_low_stack(8,2) > 2147483647-src_low_g(22))  to bound_ready }\n    state bound_ready { to literal_emit when (target == 4294967295)  to typed_emit }"
if text.count(old)!=1: raise SystemExit("OMGRFN4 composite: interval Add anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
extract_proc "$L4COMMON" src_low_primary "$T/l4-interval-primary.beta"
python3 -B - "$T/l4-interval-primary.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state parameter_refine { to parameter_advance when (src_low_g(5) == 4294967295)  to parameter_advance when (word[19600000+src_low_g(5)*32] != 1)  to parameter_advance when (word[19600008+src_low_g(5)*32] != 2)  to parameter_advance when (word[19600016+src_low_g(5)*32] != id)  to parameter_advance when (src_low_g(22) <= word[19600024+src_low_g(5)*32])  src_low_gset(22,word[19600024+src_low_g(5)*32])  to parameter_advance }"
new="""state parameter_refine { to parameter_fact when (src_low_g(5) == 4294967295)  to parameter_fact when (word[23000000+src_low_g(5)*8] == 0)  to parameter_fact when (word[23100000+id*8] == 0)  src_low_gset(21,word[23200000+id*8])  src_low_gset(22,word[23300000+id*8])  to parameter_fact }
    state parameter_fact { to arm_fact when (src_low_g(5) == 4294967295)  to arm_fact when (word[19600000+src_low_g(5)*32] != 1)  to arm_fact when (word[19600008+src_low_g(5)*32] != 2)  to arm_fact when (word[19600016+src_low_g(5)*32] != id)  to arm_fact when (src_low_g(22) <= word[19600024+src_low_g(5)*32])  src_low_gset(22,word[19600024+src_low_g(5)*32])  to arm_fact }
    state arm_fact { to parameter_advance when (word[23400016] != 1)  to parameter_advance when (word[23400024] != 1)  to parameter_advance when (word[23400032] != 2)  to parameter_advance when (word[23400040] != id)  to parameter_advance when (src_low_g(22) <= word[23400048])  src_low_gset(22,word[23400048])  to parameter_advance }"""
if text.count(old)!=1: raise SystemExit("OMGRFN4 composite: interval primary anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
sed '/^proc v3_ckir_header_check/,$d' "$L4V3" > "$T/l4-interval-v3.beta"
python3 -B - "$T/l4-interval-v3.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state index_bound { to bad when (index_high >= src_type(base_type,5))  to index_left when (src_low_g(30) >= index_tree)"
new="state index_bound { to index_left when (word[23400008] == 1)  to bad when (index_high >= src_type(base_type,5))  to index_left when (src_low_g(30) >= index_tree)"
if text.count(old)!=1: raise SystemExit("OMGRFN4 composite: interval index anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
cp "$T/l4-guarded.beta" "$T/l4-interval-guarded.beta"
python3 -B - "$T/l4-interval-guarded.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
text=text.replace("fact_hi=src_low_g(44)  to done", "fact_hi=src_low_g(44)  word[23400024]=fact_valid  word[23400032]=fact_kind  word[23400040]=fact_id  word[23400048]=fact_hi  word[23400056]=src_low_g(45)  to done",1)
text=text.replace("true_seen=1  arm_kind=1", "true_seen=1  arm_kind=1  word[23400016]=1",1)
text=text.replace("false_seen=1  arm_kind=2", "false_seen=1  arm_kind=2  word[23400016]=2",1)
text=text.replace("wild_seen=1  arm_kind=3", "wild_seen=1  arm_kind=3  word[23400016]=3",1)
p.write_text(text,encoding="ascii")
PY
for PROC in v4_skip_constant src_low_expression src_low_body; do
  extract_proc "$L4OPERATIONS" "$PROC" "$T/l4-interval-$PROC.beta"
done
python3 -B - "$T/l4-interval-src_low_body.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="src_low_gset(52,ckir_row_word(7,src_low_g(0),40,32))"
if text.count(old)!=1: raise SystemExit("OMGRFN4 composite: interval aggregate anchor drifted")
p.write_text(text.replace(old,"src_low_gset(52,0)"),encoding="ascii")
PY
cat "$ENVELOPE" "$T/l4-base.beta" "$L4MODEL" "$T/l4-interval-common.beta" \
  "$T/l4-interval-primary.beta" "$T/l4-interval-v3.beta" \
  "$T/l4-interval-guarded.beta" "$T/l4-v4_token_equal_named.beta" \
  "$T/l4-interval-v4_skip_constant.beta" \
  "$T/l4-interval-src_low_expression.beta" "$T/l4-interval-src_low_body.beta" \
  "$L4INTERVAL" > "$T/layer4-interval-all.beta"
prune_reachable "$T/layer4-interval-all.beta" "$T/layer4-interval.beta" main

# The source-result companion is physically artifact-free after composition:
# truncate the envelope before CKIR accessors, remove artifact-dependent
# helpers before pruning, then retain only main's transitive closure.
sed '/^proc omgrfn4_component_ckir_byte/,$d' "$ENVELOPE" > "$T/l4-result-envelope.beta"
filter_procs "$L4COMMON" "$T/l4-result-common.beta" \
  'ckir_u32,ckir_row_word,ckir_row_byte,ckir_bparam_word,ckir_operand,src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_expression,src_low_transition,src_low_body,src_low_block_owner,src_lower_compare_final,src_refinement_lowering_check,main'
python3 -B - "$T/l4-result-common.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="  to bad when (src_low_stack(8,2) > 2147483647-src_low_g(22))  to literal_emit"
new="  to literal_emit"
if text.count(old)!=1: raise SystemExit("OMGRFN4 composite: source-result Add anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
sed '/^proc v3_ckir_header_check/,$d' "$L4V3" > "$T/l4-result-v3.beta"
filter_procs "$T/l4-result-v3.beta" "$T/l4-result-v3-filtered.beta" \
  'v3_call_begin,v3_call_binding,v3_call_finish'
mv "$T/l4-result-v3-filtered.beta" "$T/l4-result-v3.beta"
python3 -B - "$T/l4-result-v3.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state index_bound { to bad when (index_high >= src_type(base_type,5))  to index_left when (src_low_g(30) >= index_tree)"
new="state index_bound { to index_left when (src_low_g(30) >= index_tree)"
if text.count(old)!=1: raise SystemExit("OMGRFN4 composite: source-result index anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
cat "$T/l4-result-envelope.beta" "$T/l4-base.beta" "$L4MODEL" \
  "$T/l4-result-common.beta" "$T/l4-result-v3.beta" "$L4RESULT" \
  > "$T/layer4-result-all.beta"
prune_reachable "$T/layer4-result-all.beta" "$T/layer4-result.beta" main

# Responsibility 5 deliberately remains two executables. One owns complete
# CKIR/result meaning without an ELF reader; the other independently repeats
# complete CKIR structure then reconstructs ELF without importing evaluator
# state or its result conclusion.
sed '/^proc main()/,$d' "$L5ARTIFACT" > "$T/layer5-artifact-body.beta"
sed '/^proc main()/,$d' "$L5RESULT" > "$T/layer5-result-body.beta"
sed '/^proc main()/,$d' "$L5ELF" > "$T/layer5-elf-body.beta"
cat "$ENVELOPE" "$T/layer5-artifact-body.beta" \
  "$T/layer5-result-body.beta" > "$T/layer5-result.beta"
printf '%s\n' '' 'proc main() {' \
  '    let status = omgrfn4_component_read()' \
  '    state envelope { to done when (status != 0)  status = ckir3_refinement_artifact_check()  to done }' \
  '    state done { return status }' '}' >> "$T/layer5-result.beta"
cat "$ENVELOPE" "$T/layer5-artifact-body.beta" \
  "$T/layer5-elf-body.beta" > "$T/layer5-elf.beta"
printf '%s\n' '' 'proc main() {' \
  '    let status = omgrfn4_component_read()' \
  '    state envelope { to done when (status != 0)  status = ckir3_refinement_structure_check()  to artifact }' \
  '    state artifact { to done when (status != 0)  status = ckir3_refinement_elf_check()  to done }' \
  '    state done { return status }' '}' >> "$T/layer5-elf.beta"

CHECKERS='layer1 layer2 layer3 layer4-operations layer4-roots layer4-interval layer4-result layer5-result layer5-elf'

# The source evaluator's opacity is structural, not merely an unexercised path.
# Retain only the transitive main closure, then prove the resulting persisted
# program physically contains no CKIR/ELF accessor.
python3 - "$T/layer4-result.beta" <<'PY'
from pathlib import Path
import re, sys

path = Path(sys.argv[1]); text = path.read_text(encoding="ascii")
procedures, spans, order = {}, {}, []
for match in re.finditer(r"(?m)^proc\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*\)\s*\{", text):
    depth, cursor = 1, match.end()
    while depth and cursor < len(text):
        depth += (text[cursor] == "{") - (text[cursor] == "}")
        cursor += 1
    if depth:
        raise SystemExit(f"unterminated procedure {match.group(1)}")
    name = match.group(1)
    procedures[name] = text[match.end():cursor-1]
    spans[name] = (match.start(), cursor)
    order.append(name)
if "main" not in procedures:
    raise SystemExit("OMGRFN4 same-frame composite: source-result has no main")
reachable, pending = {"main"}, ["main"]
while pending:
    caller = pending.pop()
    for callee in re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(", procedures[caller]):
        if callee in procedures and callee not in reachable:
            reachable.add(callee); pending.append(callee)
pruned = "\n\n".join(
    text[spans[name][0]:spans[name][1]].rstrip()
    for name in order if name in reachable
) + "\n"
forbidden_names = {
    "omgrfn4_component_ckir_byte", "omgrfn4_component_elf_byte",
    "refinement_ckir_byte", "refinement_elf_byte", "ckir_u32",
    "ckir_row_word", "ckir_row_byte", "ckir_operand",
    "src_low_decode_validated_ckir", "src_lower_compare_final",
    "v3_ckir_header_check", "v4_ckir_header_check",
}
forbidden = sorted(
    forbidden_names &
    (reachable | set(re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(", pruned)))
)
if forbidden:
    raise SystemExit(
        "OMGRFN4 same-frame composite: source-result contains artifact readers: "
        f"{forbidden}"
    )
required = ("v4s_parse_constant", "v4s_install", "opcode==11", "opcode==12",
            "depth>=16", "word[43400000]>=65536", "byte[32000000+clear]")
missing = [anchor for anchor in required if anchor not in pruned]
if missing:
    raise SystemExit(f"OMGRFN4 same-frame composite: source-result lost {missing}")
regions = (
    (28_000_000,29_572_864,"raw nodes"),
    (30_000_000,30_524_288,"raw children"),
    (30_600_000,30_601_024,"record scratch"),
    (30_700_000,30_765_536,"array scratch"),
    (32_000_000,32_131_072,"owner"),
    (33_000_000,37_718_592,"value frames"),
    (38_000_000,42_194_304,"place frames"),
    (43_000_000,43_032_768,"edge staging"),
    (43_100_000,43_100_128,"results"),
    (43_101_000,43_101_128,"blocks"),
    (43_200_000,43_331_072,"copy"),
    (43_400_000,43_400_008,"entry counter"),
)
for left, right in zip(sorted(regions), sorted(regions)[1:]):
    if left[1] > right[0]:
        raise SystemExit(f"OMGRFN4 same-frame composite: source-result overlap {left}/{right}")
if max(end for _, end, _ in regions) > 0x04000000:
    raise SystemExit("OMGRFN4 same-frame composite: source-result exceeds Alpha memory")
path.write_text(pruned, encoding="ascii")
PY

# Assert the composed R5 address plan itself.  The artifact/ELF stages share
# only the documented constant-root offsets and immutable image; evaluator and
# ELF work regions are disjoint at their full published capacities.
python3 - "$L5ARTIFACT" "$L5RESULT" "$L5ELF" <<'PY'
from pathlib import Path
import sys

artifact = Path(sys.argv[1]).read_text(encoding="ascii")
result = Path(sys.argv[2]).read_text(encoding="ascii")
elf = Path(sys.argv[3]).read_text(encoding="ascii")
for anchor in ("10500000", "10600000", "13000000", "13400000", "13500000", "13700000"):
    if anchor not in artifact:
        raise SystemExit(f"OMGRFN4 same-frame composite: artifact map lost {anchor}")
for anchor in ("10610000", "10614000", "10620000"):
    if anchor not in result:
        raise SystemExit(f"OMGRFN4 same-frame composite: result map lost {anchor}")
for anchor in ("10800000", "11000000", "11100000", "11400000", "13400000", "13700000"):
    if anchor not in elf:
        raise SystemExit(f"OMGRFN4 same-frame composite: ELF map lost {anchor}")
private = (
    ("frame", 1_048_576, 5_546_120),
    ("CKIR evaluator", 10_500_000, 10_630_000),
    ("ELF reconstruction", 10_800_000, 11_694_912),
    ("constant metadata", 13_000_000, 13_465_536),
    ("constant children", 13_500_000, 13_631_072),
    ("constant image", 13_700_000, 13_831_072),
)
for index, (left_name, left_start, left_end) in enumerate(private):
    if left_start >= left_end:
        raise SystemExit(f"empty memory region {left_name}")
    for right_name, right_start, right_end in private[index+1:]:
        if max(left_start, right_start) < min(left_end, right_end):
            raise SystemExit(
                f"OMGRFN4 same-frame composite: {left_name}/{right_name} memory overlap"
            )
PY

stamp_beta_compiler "$T/bc" >/dev/null
BC="$T/bc"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"

build_checker() {
  NAME=$1
  PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/$NAME.beta")
  MAX_LOCALS=$(python3 - "$T/$NAME.beta" <<'PY'
import re, sys
source = open(sys.argv[1], encoding="ascii").read()
maximum = 0
for match in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{", source, re.M):
    end = source.find("\nproc ", match.end())
    body = source[match.end():end if end >= 0 else len(source)]
    params = sum(bool(item.strip()) for item in match.group(1).split(","))
    maximum = max(maximum, params + len(re.findall(r"\blet\s+[A-Za-z_]\w*", body)))
print(maximum)
PY
)
  [ "$PROCEDURES" -le 128 ] || {
    echo "OMGRFN4 same-frame composite: $NAME has $PROCEDURES procedures" >&2
    return 1
  }
  [ "$MAX_LOCALS" -le 32 ] || {
    echo "OMGRFN4 same-frame composite: $NAME has $MAX_LOCALS locals" >&2
    return 1
  }
  observe "build-beta-$NAME" 0 120 "$T/$NAME.beta" "$T/$NAME.asm" no "$BC"
  observe "build-alpha-$NAME" 0 120 "$T/$NAME.asm" "$T/$NAME.tape" no "$ASM"
  TAPE_BYTES=$(wc -c < "$T/$NAME.tape" | tr -d ' ')
  [ "$TAPE_BYTES" -le 262140 ] || {
    echo "OMGRFN4 same-frame composite: $NAME has $TAPE_BYTES tape bytes" >&2
    return 1
  }
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME" >/dev/null 2>&1
  printf '%s\t%s\t%s\t%s\n' "$NAME" "$PROCEDURES" "$MAX_LOCALS" "$TAPE_BYTES" \
    > "$T/$NAME.resources"
}

PIDS=''
for NAME in $CHECKERS; do
  build_checker "$NAME" &
  PIDS="$PIDS $!"
done
for PID in $PIDS; do wait "$PID"; done

# Build the three Delta producers once and use exact Unicode+harness bytes for
# the single canonical carrier shared unchanged by every responsibility.
observe cargo-build 0 180 /dev/null "$T/cargo.out" yes \
  cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
PIDS=''
for SPEC in "resolver:$RESOLVER" "lowerer:$LOWERER" "backend:$BACKEND"; do
  NAME=${SPEC%%:*}
  SOURCE=${SPEC#*:}
  observe "compile-$NAME" 0 90 /dev/null "$T/compile-$NAME.out" yes \
    env DELTA_ARCH=aarch64 "$DELTA" "$SOURCE" "$T/$NAME" &
  PIDS="$PIDS $!"
done
for PID in $PIDS; do
  wait "$PID"
done
[ -x "$T/resolver" ] && [ -x "$T/lowerer" ] && [ -x "$T/backend" ] || {
  echo "OMGRFN4 same-frame composite: producer compilation failed" >&2
  exit 1
}

build_product() { # label owner machine source...
  LABEL=$1 OWNER=$2 MACHINE=$3
  shift 3
  observe "$LABEL-builder" 0 30 /dev/null "$T/$LABEL.builder" yes \
    python3 -B "$BUILDER" build "$T/$LABEL.omgc" "$OWNER" "$MACHINE" "$@"
  observe "$LABEL-resolver" 0 30 "$T/$LABEL.omgc" "$T/$LABEL.witness" no \
    "$T/resolver"
  observe "$LABEL-frame" 0 20 /dev/null "$T/$LABEL.low3" no \
    python3 -B "$LOW_FRAME" pack "$T/$LABEL.omgc" "$T/$LABEL.witness"
  observe "$LABEL-lowerer" 0 60 "$T/$LABEL.low3" "$T/$LABEL.ckir3" no \
    "$T/lowerer"
  observe "$LABEL-backend" 0 60 "$T/$LABEL.ckir3" "$T/$LABEL.elf" no \
    "$T/backend"
  observe "$LABEL-ir-validate" 0 30 /dev/null "$T/$LABEL.ir-check" no \
    python3 -B "$IR_REFERENCE" validate "$T/$LABEL.ckir3"
  observe "$LABEL-ir-run" 0 30 /dev/null "$T/$LABEL.result" no \
    python3 -B "$IR_REFERENCE" run "$T/$LABEL.ckir3"
  observe "$LABEL-elf-check" 0 30 /dev/null "$T/$LABEL.elf-check" no \
    python3 -B "$ELF_REFERENCE" check "$T/$LABEL.ckir3" "$T/$LABEL.elf"
}

build_product canonical UnicodeTables bootstrap_constant_aggregate_probe \
  "$UNICODE" "$HARNESS"
[ "$(tr -d '\n' < "$T/canonical.result")" = 70 ] || {
  echo "OMGRFN4 same-frame composite: canonical independent result drifted" >&2
  exit 1
}
[ "$(wc -c < "$T/canonical.omgc" | tr -d ' ')" -eq 84140 ]
[ "$(wc -c < "$T/canonical.witness" | tr -d ' ')" -eq 3004 ]
[ "$(wc -c < "$T/canonical.ckir3" | tr -d ' ')" -eq 94172 ]
[ "$(wc -c < "$T/canonical.elf" | tr -d ' ')" -eq 24576 ]
observe canonical-pack 0 20 /dev/null "$T/canonical.rfn" no \
  python3 -B "$PACKER" "$T/canonical.omgc" "$T/canonical.witness" \
  "$T/canonical.ckir3" "$T/canonical.elf" --result 70
python3 - "$T/canonical.rfn" "$T/canonical.sha256" <<'PY'
from pathlib import Path
import hashlib, sys
raw = Path(sys.argv[1]).read_bytes()
assert len(raw) == 205_932
Path(sys.argv[2]).write_text(hashlib.sha256(raw).hexdigest()+"\n", encoding="ascii")
PY

check() { # checker expected input case
  NAME=$1 EXPECTED=$2 INPUT=$3 CASE=$4
  # Checker observations consume immutable carriers and publish only private,
  # case-specific files. Queue them so the complete matrix can use a bounded
  # worker set once every carrier has been constructed. Tabs/newlines cannot
  # occur in these repository-owned labels or temporary paths.
  printf '%s\t%s\t%s\t%s\n' "$NAME" "$EXPECTED" "$INPUT" "$CASE" \
    >> "$T/check-queue.tsv"
}

for NAME in $CHECKERS; do check "$NAME" 0 "$T/canonical.rfn" canonical; done

# A nearby valid Unicode product changes only the successful source result.
# It exercises the same general lowering/backend path and remains independently
# valid before any cross-pair is assembled.
python3 - "$HARNESS" "$T/variant-harness.omg" <<'PY'
from pathlib import Path
import sys
raw = Path(sys.argv[1]).read_bytes()
old = b"state pass(&mut self) { 70 }"
assert raw.count(old) == 1
Path(sys.argv[2]).write_bytes(raw.replace(old, b"state pass(&mut self) { 72 }"))
PY
build_product variant UnicodeTables bootstrap_constant_aggregate_probe \
  "$UNICODE" "$T/variant-harness.omg"
[ "$(tr -d '\n' < "$T/variant.result")" = 72 ] || exit 1

# A fully renamed source/witness pair supplies independently valid source and
# witness components whose declaration identities cannot be cross-paired.
python3 - "$UNICODE" "$HARNESS" "$T/renamed-unicode.omg" "$T/renamed-harness.omg" <<'PY'
from pathlib import Path
import sys
for source, target in ((sys.argv[1], sys.argv[3]), (sys.argv[2], sys.argv[4])):
    raw = Path(source).read_bytes()
    for old, new in (
        (b"UnicodeRange", b"RuneBounds"),
        (b"UnicodeTables", b"RuneCatalog"),
        (b"initialize", b"seed"),
        (b"is_xid_start", b"starts_here"),
        (b"is_xid_continue", b"continues_here"),
        (b"bootstrap_constant_aggregate_probe", b"alternate_probe"),
    ):
        raw = raw.replace(old, new)
    Path(target).write_bytes(raw)
PY
observe renamed-builder 0 30 /dev/null "$T/renamed.builder" yes \
  python3 -B "$BUILDER" build "$T/renamed.omgc" RuneCatalog \
  alternate_probe "$T/renamed-unicode.omg" "$T/renamed-harness.omg"
observe renamed-resolver 0 30 "$T/renamed.omgc" "$T/renamed.witness" no "$T/resolver"
cmp -s "$T/renamed.witness" "$T/canonical.witness" && {
  echo "OMGRFN4 same-frame composite: renamed witness is not a distinct cross-pair control" >&2
  exit 1
}

pack_cross() { # label comp witness ckir elf result
  LABEL=$1 COMP=$2 WITNESS=$3 CKIR=$4 ELF=$5 RESULT=$6
  observe "$LABEL-pack" 0 20 /dev/null "$T/$LABEL.rfn" no \
    python3 -B "$PACKER" "$COMP" "$WITNESS" "$CKIR" "$ELF" --result "$RESULT"
}

pack_cross renamed-resolution "$T/renamed.omgc" "$T/renamed.witness" \
  "$T/canonical.ckir3" "$T/canonical.elf" 70
check layer1 0 "$T/renamed-resolution.rfn" renamed-source-custody
check layer2 0 "$T/renamed-resolution.rfn" renamed-resolution

pack_cross source-cross "$T/renamed.omgc" "$T/canonical.witness" \
  "$T/canonical.ckir3" "$T/canonical.elf" 70
check layer1 0 "$T/source-cross.rfn" source-cross
check layer2 251 "$T/source-cross.rfn" source-cross
check layer3 0 "$T/source-cross.rfn" source-cross-opaque
check layer4-operations 251 "$T/source-cross.rfn" source-cross
check layer4-roots 251 "$T/source-cross.rfn" source-span-cross
check layer4-interval 251 "$T/source-cross.rfn" source-cross
check layer4-result 251 "$T/source-cross.rfn" source-cross
check layer5-result 0 "$T/source-cross.rfn" source-cross-opaque
check layer5-elf 0 "$T/source-cross.rfn" source-cross-opaque

pack_cross witness-cross "$T/canonical.omgc" "$T/renamed.witness" \
  "$T/canonical.ckir3" "$T/canonical.elf" 70
check layer1 0 "$T/witness-cross.rfn" witness-cross-opaque
check layer2 251 "$T/witness-cross.rfn" witness-cross
check layer3 0 "$T/witness-cross.rfn" witness-source-spans-opaque
check layer4-operations 251 "$T/witness-cross.rfn" witness-cross
check layer4-roots 251 "$T/witness-cross.rfn" witness-source-span-cross
check layer4-interval 251 "$T/witness-cross.rfn" witness-cross
check layer4-result 251 "$T/witness-cross.rfn" witness-cross
check layer5-result 0 "$T/witness-cross.rfn" witness-cross-opaque
check layer5-elf 0 "$T/witness-cross.rfn" witness-cross-opaque

pack_cross ckir-cross "$T/canonical.omgc" "$T/canonical.witness" \
  "$T/variant.ckir3" "$T/canonical.elf" 70
check layer1 0 "$T/ckir-cross.rfn" ckir-cross-opaque
check layer2 0 "$T/ckir-cross.rfn" ckir-cross-opaque
check layer3 0 "$T/ckir-cross.rfn" ckir-cross-intrinsic
check layer4-operations 251 "$T/ckir-cross.rfn" ckir-cross
check layer4-roots 0 "$T/ckir-cross.rfn" ckir-nonroot-opaque
check layer4-interval 0 "$T/ckir-cross.rfn" artifact-opaque
check layer4-result 0 "$T/ckir-cross.rfn" ckir-cross-opaque
check layer5-result 251 "$T/ckir-cross.rfn" ckir-cross-result
check layer5-elf 251 "$T/ckir-cross.rfn" ckir-cross-before-elf

pack_cross artifact-source-cross "$T/canonical.omgc" "$T/canonical.witness" \
  "$T/variant.ckir3" "$T/variant.elf" 72
check layer1 0 "$T/artifact-source-cross.rfn" artifact-source-cross-opaque
check layer2 0 "$T/artifact-source-cross.rfn" artifact-source-cross-opaque
check layer3 0 "$T/artifact-source-cross.rfn" artifact-source-cross-intrinsic
check layer4-operations 251 "$T/artifact-source-cross.rfn" artifact-source-cross
check layer4-roots 0 "$T/artifact-source-cross.rfn" artifact-nonroot-opaque
check layer4-interval 0 "$T/artifact-source-cross.rfn" artifact-opaque
check layer4-result 251 "$T/artifact-source-cross.rfn" artifact-source-cross
check layer5-result 0 "$T/artifact-source-cross.rfn" artifact-source-cross
check layer5-elf 0 "$T/artifact-source-cross.rfn" artifact-source-cross

pack_cross elf-cross "$T/canonical.omgc" "$T/canonical.witness" \
  "$T/canonical.ckir3" "$T/variant.elf" 70
for NAME in layer1 layer2 layer3 layer4-operations layer4-roots layer4-interval layer4-result layer5-result; do
  check "$NAME" 0 "$T/elf-cross.rfn" elf-cross-opaque
done
check layer5-elf 251 "$T/elf-cross.rfn" elf-cross

pack_cross result-cross "$T/canonical.omgc" "$T/canonical.witness" \
  "$T/canonical.ckir3" "$T/canonical.elf" 72
for NAME in layer1 layer2 layer3 layer4-operations layer4-roots layer4-interval; do
  check "$NAME" 0 "$T/result-cross.rfn" result-cross-opaque
done
for NAME in layer4-result layer5-result; do
  check "$NAME" 251 "$T/result-cross.rfn" result-cross
done
check layer5-elf 0 "$T/result-cross.rfn" result-cross-opaque

# Whole-component opacity controls use arbitrary nonempty bytes only where the
# owning responsibility promises not to read that component.
printf opaque-witness > "$T/opaque.witness"
printf opaque-ckir3 > "$T/opaque.ckir3"
printf opaque-elf > "$T/opaque.elf"
printf opaque-omgcomp > "$T/opaque.omgc"
pack_cross opaque-later "$T/canonical.omgc" "$T/opaque.witness" \
  "$T/opaque.ckir3" "$T/opaque.elf" 71
check layer1 0 "$T/opaque-later.rfn" whole-later-opacity
pack_cross r2-opaque "$T/canonical.omgc" "$T/canonical.witness" \
  "$T/opaque.ckir3" "$T/opaque.elf" 71
check layer2 0 "$T/r2-opaque.rfn" artifact-result-opacity
pack_cross r4-source-opaque "$T/canonical.omgc" "$T/canonical.witness" \
  "$T/opaque.ckir3" "$T/opaque.elf" 70
check layer4-interval 0 "$T/r4-source-opaque.rfn" artifact-result-opacity
check layer4-result 0 "$T/r4-source-opaque.rfn" artifact-opacity
pack_cross r5-result-opaque-front "$T/opaque.omgc" "$T/opaque.witness" \
  "$T/canonical.ckir3" "$T/opaque.elf" 70
check layer5-result 0 "$T/r5-result-opaque-front.rfn" front-and-elf-opacity
pack_cross r5-elf-opaque-front "$T/opaque.omgc" "$T/opaque.witness" \
  "$T/canonical.ckir3" "$T/canonical.elf" 70
check layer5-elf 0 "$T/r5-elf-opaque-front.rfn" front-opacity

# Byte-local mutations preserve the exact carrier extent and isolate source,
# role-3 witness, opcode-11 root, ELF, and claimed-result ownership.
python3 - "$T/canonical.rfn" "$T" <<'PY'
from pathlib import Path
import struct, sys

frame = Path(sys.argv[1]).read_bytes(); out = Path(sys.argv[2])
magic, version, flags, cn, wn, kn, en, result, exit_code = struct.unpack_from("<8s8I", frame)
assert (magic, version, flags, result, exit_code) == (b"OMGRFN4\0", 4, 1, 70, 70)
comp_at=40; witness_at=comp_at+cn; ckir_at=witness_at+wn; elf_at=ckir_at+kn

changed=bytearray(frame)
needle=b"machine UnicodeTables::bootstrap_constant_aggregate_probe"
assert frame.count(needle)==1
changed[frame.index(needle)] = ord("?")
out.joinpath("source-syntax.rfn").write_bytes(changed)

witness=frame[witness_at:ckir_at]
counts=struct.unpack_from("<11I",witness,20)
bases=[]; cursor=72
for count,stride in zip(counts,(36,48,28,28,24,24,24,40,24,40,24)):
    bases.append(cursor); cursor += count*stride
role3=[bases[2]+i*28 for i in range(counts[2]) if witness[bases[2]+i*28+8]==3]
assert role3
changed=bytearray(frame)
target=struct.unpack_from("<I",changed,witness_at+role3[0]+20)[0]
struct.pack_into("<I",changed,witness_at+role3[0]+20,(target+1)%counts[3])
out.joinpath("witness-target.rfn").write_bytes(changed)

ckir=frame[ckir_at:elf_at]
header=struct.unpack_from("<8sHHHH16I",ckir)
assert header[:4]==(b"OMGCKIR\0",3,0,1)
raw_counts=header[7:]
names=("types","records","fields","machines","mparams","blocks","bparams",
       "operations","operands","terms","values","places","constants","children")
counts3=dict(zip(names,raw_counts))
order=(("types",24),("records",20),("fields",16),("machines",36),
       ("mparams",20),("blocks",32),("bparams",20),("constants",24),
       ("children",4),("operations",40),("operands",4),("terms",44))
bases3={}; cursor=80
for name,stride in order:
    bases3[name]=cursor; cursor += counts3[name]*stride
assert cursor==len(ckir)
op11=next(bases3["operations"]+i*40 for i in range(counts3["operations"])
          if ckir[bases3["operations"]+i*40+12]==11)
changed=bytearray(frame)
struct.pack_into("<I",changed,ckir_at+op11+32,0xFFFF_FFFF)
out.joinpath("opcode11-root.rfn").write_bytes(changed)

changed=bytearray(frame); changed[elf_at+en-1] ^= 1
out.joinpath("elf-byte.rfn").write_bytes(changed)
changed=bytearray(frame); struct.pack_into("<II",changed,32,72,72)
out.joinpath("result-byte.rfn").write_bytes(changed)
PY

check layer1 0 "$T/source-syntax.rfn" source-content-opaque
check layer2 251 "$T/source-syntax.rfn" source-syntax
check layer3 0 "$T/source-syntax.rfn" source-content-opaque
check layer4-operations 0 "$T/source-syntax.rfn" declaration-syntax-opaque
check layer4-roots 0 "$T/source-syntax.rfn" declaration-syntax-opaque
check layer4-interval 0 "$T/source-syntax.rfn" declaration-syntax-opaque
check layer4-result 0 "$T/source-syntax.rfn" declaration-syntax-opaque
check layer5-result 0 "$T/source-syntax.rfn" source-content-opaque
check layer5-elf 0 "$T/source-syntax.rfn" source-content-opaque

check layer1 0 "$T/witness-target.rfn" witness-target-opaque
check layer2 251 "$T/witness-target.rfn" witness-target
check layer3 0 "$T/witness-target.rfn" witness-binding-opaque
check layer4-operations 251 "$T/witness-target.rfn" witness-target
check layer4-roots 0 "$T/witness-target.rfn" witness-role3-opaque
check layer4-interval 251 "$T/witness-target.rfn" witness-target
check layer4-result 0 "$T/witness-target.rfn" unused-role3-opaque
check layer5-result 0 "$T/witness-target.rfn" witness-target-opaque
check layer5-elf 0 "$T/witness-target.rfn" witness-target-opaque

for NAME in layer1 layer2 layer3 layer4-result; do
  check "$NAME" 0 "$T/opcode11-root.rfn" opcode11-root-opaque
done
check layer4-operations 0 "$T/opcode11-root.rfn" opcode11-root-opaque
check layer4-interval 0 "$T/opcode11-root.rfn" artifact-opaque
for NAME in layer4-roots layer5-result layer5-elf; do
  check "$NAME" 251 "$T/opcode11-root.rfn" opcode11-root
done

for NAME in layer1 layer2 layer3 layer4-operations layer4-roots layer4-interval layer4-result layer5-result; do
  check "$NAME" 0 "$T/elf-byte.rfn" elf-byte-opaque
done
check layer5-elf 251 "$T/elf-byte.rfn" elf-byte

for NAME in layer1 layer2 layer3 layer4-operations layer4-roots layer4-interval; do
  check "$NAME" 0 "$T/result-byte.rfn" result-byte-opaque
done
for NAME in layer4-result layer5-result; do
  check "$NAME" 251 "$T/result-byte.rfn" result-byte
done
check layer5-elf 0 "$T/result-byte.rfn" result-byte-opaque

python3 - "$T/canonical.rfn" "$T/canonical.sha256" <<'PY'
from pathlib import Path
import hashlib, sys
expected=Path(sys.argv[2]).read_text(encoding="ascii").strip()
actual=hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest()
if actual != expected:
    raise SystemExit("OMGRFN4 same-frame composite: canonical carrier was mutated")
PY

# Each queued observation remains an invocation of the same timeout/status/
# empty-output runner. A fixed upper bound prevents the large persisted Beta
# checkers from oversubscribing memory. Per-job timing files are joined in
# declaration order, and the first failing declaration is reported even when a
# later concurrent observation happens to finish first.
python3 -B - "$T" "${OMEGA_OMGRFN4_CHECK_JOBS:-4}" <<'PY'
from pathlib import Path
import subprocess, sys, time

root = Path(sys.argv[1])
try:
    workers = int(sys.argv[2])
except ValueError:
    raise SystemExit("OMGRFN4 same-frame composite: checker parallelism must be an integer")
if not 1 <= workers <= 4:
    raise SystemExit("OMGRFN4 same-frame composite: checker parallelism must be within 1..4")
root.joinpath("check-workers").write_text(f"{workers}\n", encoding="ascii")

jobs = []
for index, line in enumerate(root.joinpath("check-queue.tsv").read_text(encoding="ascii").splitlines()):
    fields = line.split("\t")
    if len(fields) != 4:
        raise SystemExit(f"OMGRFN4 same-frame composite: malformed queued check {index}")
    name, expected, input_path, case = fields
    label = f"check-{name}-{case}"
    jobs.append({
        "index": index,
        "label": label,
        "command": [
            sys.executable, str(root / "run.py"), label, expected, "60", input_path,
            str(root / f"{name}-{case}.out"), "yes",
            str(root / f"check-{index}.timing"), str(root / name),
        ],
    })

active = {}
next_job = 0
failures = []

def launch(job):
    process = subprocess.Popen(job["command"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    active[process] = job

while next_job < len(jobs) and len(active) < workers:
    launch(jobs[next_job]); next_job += 1

while active:
    completed = [process for process in active if process.poll() is not None]
    if not completed:
        time.sleep(0.01)
        continue
    for process in sorted(completed, key=lambda item: active[item]["index"]):
        job = active.pop(process)
        stdout, stderr = process.communicate()
        if process.returncode != 0:
            failures.append((job["index"], stdout, stderr))
    if not failures:
        while next_job < len(jobs) and len(active) < workers:
            launch(jobs[next_job]); next_job += 1

with root.joinpath("timings.tsv").open("a", encoding="ascii") as report:
    for job in jobs[:next_job]:
        timing = root / f"check-{job['index']}.timing"
        if timing.exists():
            report.write(timing.read_text(encoding="ascii"))

if failures:
    _, stdout, stderr = min(failures, key=lambda item: item[0])
    if stdout:
        sys.stdout.buffer.write(stdout)
    if stderr:
        sys.stderr.buffer.write(stderr)
    raise SystemExit(1)
PY

for NAME in $CHECKERS; do cat "$T/$NAME.resources"; done > "$T/resources.tsv"
python3 - "$T/timings.tsv" "$T/resources.tsv" "$T/canonical.rfn" "$T/started" <<'PY'
from collections import defaultdict
from pathlib import Path
import sys, time

checker_names=("layer1","layer2","layer3","layer4-operations","layer4-roots","layer4-interval","layer4-result",
               "layer5-result","layer5-elf")
rows=[]; phases=defaultdict(float); builds=defaultdict(float); checks=defaultdict(float)
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    seconds,label=line.split("\t",1); seconds=float(seconds)
    rows.append((seconds,label))
    matched=False
    for name in checker_names:
        if label in (f"build-beta-{name}",f"build-alpha-{name}"):
            builds[name]+=seconds; matched=True; break
        if label.startswith(f"check-{name}-"):
            checks[name]+=seconds; matched=True; break
    if matched:
        continue
    if label.startswith("compile-") or label=="cargo-build":
        phases["build"] += seconds
    else:
        phases["producer"] += seconds
resources=[]
for line in Path(sys.argv[2]).read_text(encoding="ascii").splitlines():
    name,procs,locals_,tape=line.split("\t")
    resources.append(f"{name}={procs}p/{locals_}l/{tape}b")
slow=max(rows)
wall=time.time()-float(Path(sys.argv[4]).read_text(encoding="ascii"))
print(
    "OMGRFN4 same-frame composite timings: "
    + " ".join(f"{name}=build{builds[name]:.3f}s/run{checks[name]:.3f}s"
               for name in checker_names) + " "
    + f"producer-build-command-total={phases['build']:.3f}s "
    f"producer-command-total={phases['producer']:.3f}s "
    f"checker-command-total={sum(checks.values()):.3f}s "
    f"checker-parallelism={Path(sys.argv[4]).parent.joinpath('check-workers').read_text(encoding='ascii').strip()} "
    f"slowest={slow[1]}:{slow[0]:.3f}s wall={wall:.3f}s"
)
print("OMGRFN4 same-frame composite resources: " + " ".join(resources))
print(
    "OMGRFN4 same-frame composite: all five responsibilities accepted one unchanged "
    f"{Path(sys.argv[3]).stat().st_size}-byte exact Unicode+harness carrier; "
    "source/witness/CKIR3/ELF/result cross-pairs, phase opacity, and local mutations passed"
)
PY
