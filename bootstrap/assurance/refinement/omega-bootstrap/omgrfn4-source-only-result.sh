#!/usr/bin/env sh
# Complete OMGRFN4 responsibility-4 physically artifact-free source result.
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
  *) echo "OMGRFN4 source-only result: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN4 source-only result: skipped ($TOOL absent)"
    exit 0
  }
done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
ENVELOPE=$R/omgrfn4-component-envelope.beta
BASE_MODEL=$R/omgrfn2-resolved-body-model.beta
MODEL=$R/omgrfn4-source-body-model.beta
COMMON=$R/ckir-refinement-source-lowering.beta
V3=$R/omgrfn3-resolved-body-lowering.beta
RESULT=$R/omgrfn4-source-only-result.beta
PACKER=$R/omgrfn4_bundle.py
BUILDER=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py
RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp
REFERENCE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/resolution_handoff_reference.py
FIXTURES=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir3-constant-aggregates
UNICODE=$OMEGA_REPO_ROOT/compiler/psi/generated/unicode_tables.omg
for REQUIRED in "$ENVELOPE" "$BASE_MODEL" "$MODEL" "$COMMON" "$V3" "$RESULT" \
  "$PACKER" "$BUILDER" "$RESOLVER" "$REFERENCE" \
  "$FIXTURES/unicode-harness.omg" "$FIXTURES/renamed-reordered-nested.omg" "$UNICODE"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN4 source-only result: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN4_SOURCE_RESULT_TEMP:-0}" = 1 ]; then
  echo "OMGRFN4 source-only result: retained $T" >&2
else
  trap 'rm -rf "$T"' EXIT
fi
: > "$T/timings.tsv"

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
procedures={}; spans={}
for match in starts:
    depth=1; cursor=match.end()
    while depth and cursor<len(source):
        depth+=(source[cursor]=="{")-(source[cursor]=="}"); cursor+=1
    if depth: raise SystemExit(f"unterminated procedure {match.group(1)}")
    procedures[match.group(1)]=source[match.start():cursor].rstrip()+"\n"
    spans[match.group(1)]=match.start()
reachable=set(); pending=[sys.argv[3]]
while pending:
    name=pending.pop()
    if name in reachable: continue
    if name not in procedures: raise SystemExit(f"missing reachable procedure {name}")
    reachable.add(name)
    for called in re.findall(r"\b([A-Za-z_]\w*)\s*\(",procedures[name]):
        if called in procedures and called not in reachable: pending.append(called)
Path(sys.argv[2]).write_text(
    "\n".join(procedures[name] for name in sorted(reachable,key=spans.get)),encoding="ascii")
PY
}

# Only exact frame, OMGCOMP, and witness access survive. CKIR/ELF accessors are
# cut out before dependency pruning, so opacity is a physical property.
sed '/^proc omgrfn4_component_ckir_byte/,$d' "$ENVELOPE" > "$T/envelope-source.beta"
sed 's/omgrfn2_component/omgrfn4_component/g' "$BASE_MODEL" > "$T/base-model-all.beta"
filter_procs "$T/base-model-all.beta" "$T/base-model.beta" \
  'l4_model_declarations,l4_model_types_records_fields,l4_model_machines_blocks,l4_model_prepare'
filter_procs "$COMMON" "$T/common-source.beta" \
  'ckir_u32,ckir_row_word,ckir_row_byte,ckir_bparam_word,ckir_operand,src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_expression,src_low_transition,src_low_body,src_low_block_owner,src_lower_compare_final,src_refinement_lowering_check,main'
sed '/^proc v3_ckir_header_check/,$d' "$V3" > "$T/v3-source-prefix.beta"
filter_procs "$T/v3-source-prefix.beta" "$T/v3-source-prefix-filtered.beta" \
  'v3_call_begin,v3_call_binding,v3_call_finish'
mv "$T/v3-source-prefix-filtered.beta" "$T/v3-source-prefix.beta"
python3 -B - "$T/v3-source-prefix.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="state index_bound { to bad when (index_high >= src_type(base_type,5))  to index_left when (src_low_g(30) >= index_tree)"
new="state index_bound { to index_left when (src_low_g(30) >= index_tree)"
if text.count(old)!=1: raise SystemExit("OMGRFN4 source-only result: V3 index-bound anchor drifted")
text=text.replace(old,new)
p.write_text(text,encoding="ascii")
PY
python3 -B - "$T/common-source.beta" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); text=p.read_text(encoding="ascii")
old="  to bad when (src_low_stack(8,2) > 2147483647-src_low_g(22))  to literal_emit"
new="  to literal_emit"
if text.count(old)!=1: raise SystemExit("OMGRFN4 source-only result: common Add-bound anchor drifted")
p.write_text(text.replace(old,new),encoding="ascii")
PY
cat "$T/envelope-source.beta" "$T/base-model.beta" "$MODEL" \
  "$T/common-source.beta" "$T/v3-source-prefix.beta" \
  "$RESULT" > "$T/source-result-all.beta"
prune_reachable "$T/source-result-all.beta" "$T/source-result.beta" main

python3 -B - "$T/source-result.beta" <<'PY'
from pathlib import Path
import re,sys
text=Path(sys.argv[1]).read_text(encoding="ascii")
procedures=set(re.findall(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\(",text))
forbidden={
 "omgrfn4_component_ckir_byte","omgrfn4_component_elf_byte",
 "refinement_ckir_byte","refinement_elf_byte","ckir_u32","ckir_row_word",
 "ckir_row_byte","ckir_operand","src_low_decode_validated_ckir",
 "src_lower_compare_final","v3_ckir_header_check","v4_ckir_header_check",
}
bad=sorted(procedures & forbidden)
if bad: raise SystemExit(f"source-result contains artifact procedures: {bad}")
called=set(re.findall(r"\b([A-Za-z_]\w*)\s*\(",text))
bad=sorted(called & forbidden)
if bad: raise SystemExit(f"source-result calls artifact accessors: {bad}")
required=("v4s_parse_constant","v4s_install","opcode==11","opcode==12",
          "depth>=16","word[43400000]>=65536","byte[32000000+clear]")
missing=[item for item in required if item not in text]
if missing: raise SystemExit(f"source-result semantic/resource anchors absent: {missing}")
regions=[
 (28000000,29572864,"raw-nodes"),(30000000,30524288,"raw-children"),
 (30600000,30601024,"record-scratch"),(30700000,30765536,"array-scratch"),
 (32000000,32131072,"owner"),(33000000,37718592,"values"),
 (38000000,42194304,"places"),(43000000,43032768,"edge-stage"),
 (43100000,43100128,"results"),(43101000,43101128,"blocks"),
 (43200000,43331072,"copy"),(43400000,43400008,"counter"),
]
for left,right in zip(sorted(regions),sorted(regions)[1:]):
    if left[1]>right[0]: raise SystemExit(f"source-result memory overlap: {left}/{right}")
if max(end for _,end,_ in regions)>0x04000000: raise SystemExit("source-result memory exceeds Alpha")
PY

PROCEDURES=$(awk '/^proc / { n++ } END { print n+0 }' "$T/source-result.beta")
MAX_LOCALS=$(python3 -B - "$T/source-result.beta" <<'PY'
import re,sys
s=open(sys.argv[1],encoding="ascii").read(); maximum=0
for m in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{",s,re.M):
    end=s.find("\nproc ",m.end()); body=s[m.end():end if end>=0 else len(s)]
    maximum=max(maximum,sum(bool(x.strip()) for x in m.group(1).split(","))+len(re.findall(r"\blet\s+[A-Za-z_]\w*",body)))
print(maximum)
PY
)
[ "$PROCEDURES" -le 128 ] || { echo "OMGRFN4 source-only result: $PROCEDURES procedures" >&2; exit 1; }
[ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN4 source-only result: $MAX_LOCALS locals" >&2; exit 1; }

timed_run() { # timeout input output expected label command...
  LIMIT=$1 INPUT=$2 OUTPUT=$3 EXPECTED=$4 LABEL=$5; shift 5
  python3 -B - "$LIMIT" "$INPUT" "$OUTPUT" "$EXPECTED" "$LABEL" "$T/timings.tsv" "$@" <<'PY'
from pathlib import Path
import os,signal,subprocess,sys,time
limit,input_path,output_path,expected,label,timings,*command=sys.argv[1:]
source=subprocess.DEVNULL if input_path=="-" else open(input_path,"rb")
try:
    started=time.monotonic()
    process=subprocess.Popen(command,stdin=source,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    try: stdout,stderr=process.communicate(timeout=float(limit))
    except subprocess.TimeoutExpired:
        os.killpg(process.pid,signal.SIGKILL); stdout,stderr=process.communicate()
        raise SystemExit(f"OMGRFN4 source-only result: {label} timed out after {limit}s")
    elapsed=time.monotonic()-started
finally:
    if source is not subprocess.DEVNULL: source.close()
Path(output_path).write_bytes(stdout)
Path(output_path+".stderr").write_bytes(stderr)
with open(timings,"a",encoding="ascii") as report: report.write(f"{elapsed:.6f}\t{label}\n")
if process.returncode!=int(expected):
    if stderr: sys.stderr.buffer.write(stderr[-4096:])
    raise SystemExit(f"OMGRFN4 source-only result: {label} returned {process.returncode}, expected {expected}")
if stdout and label not in ("beta-build","alpha-assemble") and not label.endswith("-resolver"):
    raise SystemExit(f"OMGRFN4 source-only result: {label} published output")
PY
}

stamp_beta_compiler "$T/bc" >/dev/null
timed_run 120 "$T/source-result.beta" "$T/source-result.asm" 0 beta-build "$T/bc"
timed_run 120 "$T/source-result.asm" "$T/source-result.tape" 0 alpha-assemble "$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
TAPE_BYTES=$(wc -c < "$T/source-result.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || { echo "OMGRFN4 source-only result: tape $TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/source-result.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$T/source-result" >/dev/null 2>&1

timed_run 120 - "$T/cargo.out" 0 cargo-build cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
timed_run 60 - "$T/compile-resolver.out" 0 compile-resolver env DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver"

observe() { # input expected label timeout
  timed_run "$4" "$1" "$T/$3.out" "$2" "$3" "$T/source-result"
}
resolve_case() { # omgcomp witness label
  timed_run 45 "$1" "$2" 0 "$3-resolver" "$T/resolver"
}
pack_case() { # omgcomp witness ckir elf result output
  python3 -B "$PACKER" "$1" "$2" "$3" "$4" --result "$5" > "$6"
}

printf opaque-source-result-ckir > "$T/opaque.ckir"
printf opaque-source-result-elf > "$T/opaque.elf"
python3 -B "$BUILDER" build "$T/unicode.omgc" UnicodeTables bootstrap_constant_aggregate_probe \
  "$UNICODE" "$FIXTURES/unicode-harness.omg"
resolve_case "$T/unicode.omgc" "$T/unicode.witness" unicode
pack_case "$T/unicode.omgc" "$T/unicode.witness" "$T/opaque.ckir" "$T/opaque.elf" 70 "$T/unicode.rfn"
observe "$T/unicode.rfn" 0 unicode-result-70 120

python3 -B "$BUILDER" build "$T/compact.omgc" AggregateProbe run \
  "$FIXTURES/renamed-reordered-nested.omg"
resolve_case "$T/compact.omgc" "$T/compact.witness" compact
pack_case "$T/compact.omgc" "$T/compact.witness" "$T/opaque.ckir" "$T/opaque.elf" 70 "$T/compact.rfn"
observe "$T/compact.rfn" 0 compact-nested-result-70 30

# The source proposition owns the full-width result claim, while both artifact
# components remain physically opaque.
pack_case "$T/unicode.omgc" "$T/unicode.witness" "$T/opaque.ckir" "$T/opaque.elf" 71 "$T/wrong-result.rfn"
observe "$T/wrong-result.rfn" 251 claimed-result-mutation 120
printf a-different-nonempty-ckir > "$T/other.ckir"
printf a-different-nonempty-elf > "$T/other.elf"
pack_case "$T/unicode.omgc" "$T/unicode.witness" "$T/other.ckir" "$T/other.elf" 70 "$T/artifact-opaque.rfn"
observe "$T/artifact-opaque.rfn" 0 ckir-elf-opacity 120
python3 -B - "$T/unicode.witness" "$T/bad.witness" <<'PY'
from pathlib import Path
import sys
raw=bytearray(Path(sys.argv[1]).read_bytes()); raw[0]^=1; Path(sys.argv[2]).write_bytes(raw)
PY
pack_case "$T/unicode.omgc" "$T/bad.witness" "$T/opaque.ckir" "$T/opaque.elf" 70 "$T/bad-witness.rfn"
observe "$T/bad-witness.rfn" 251 witness-mutation 30

# Genuine source/witness families exercise the bounds in this exact executable;
# the earlier focused boundary checker remains a separate corroborating claim.
python3 -B - "$REFERENCE" "$T" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0,str(Path(sys.argv[1]).parent))
from resolution_handoff_reference import one_source
out=Path(sys.argv[2])
def chain(count):
    parts=["module app;","data Probe {}"]
    for index in range(count):
        name="run" if index==0 else f"m{index}"
        body="70" if index+1==count else f"self.m{index+1}()"
        parts.append(f"machine Probe::{name}(&mut self) -> u32 {{ {body} }}")
    return "\n".join(parts)+"\n"
def cyclic(bound):
    return f'''module app;
data Probe {{}}
machine Probe::run(&mut self) -> u32 {{
 transition {{ _ -> loop(0) }}
 state loop(&mut self, index: u32 in Trapping) {{
  transition index < {bound} {{
   true -> loop(index + 1)
   _ -> pass()
  }}
 }}
 state pass(&mut self) {{ 70 }}
}}
'''
cases={"frames-16":chain(16),"frames-17":chain(17),
       "entries-65536":cyclic(65533),"entries-65537":cyclic(65534)}
for name,source in cases.items():
    (out/f"{name}.omgc").write_bytes(one_source(source,module="app",owner="Probe"))
PY
for CASE in frames-16 frames-17 entries-65536 entries-65537; do
  resolve_case "$T/$CASE.omgc" "$T/$CASE.witness" "$CASE"
  pack_case "$T/$CASE.omgc" "$T/$CASE.witness" "$T/opaque.ckir" "$T/opaque.elf" 70 "$T/$CASE.rfn"
done
observe "$T/frames-16.rfn" 0 frames-16 30
observe "$T/frames-17.rfn" 252 frames-17 30
observe "$T/entries-65536.rfn" 0 entries-65536 30
observe "$T/entries-65537.rfn" 252 entries-65537 30

python3 -B - "$T/timings.tsv" <<'PY'
from pathlib import Path
import sys
rows=[]
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    seconds,label=line.split("\t",1); rows.append((float(seconds),label))
print("OMGRFN4 source-only result timings: "+" ".join(f"{label}={seconds:.3f}s" for seconds,label in rows))
PY
echo "OMGRFN4 source-only result: exact Unicode+harness and compact nested source meaning, raw aggregate installation, < and <=, guardless/cyclic control, 16/17 frames, 65536/65537 entries, result/witness mutations, CKIR/ELF opacity, and 0/251/252 passed ($PROCEDURES/128 procedures; $MAX_LOCALS/32 locals; $TAPE_BYTES/262140 tape bytes)"
