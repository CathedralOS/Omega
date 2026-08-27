#!/usr/bin/env sh
# Focused persisted-Beta OMGRFN3 layer-4 call lowering and source-only result.
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
  *) echo "OMGRFN3 layer 4: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN3 layer 4: skipped ($TOOL absent)"
    exit 0
  }
done

ENVELOPE="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3-component-envelope.beta"
MODEL="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2-resolved-body-model.beta"
COMMON="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-lowering.beta"
V3LOW="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3-resolved-body-lowering.beta"
RESULT_HELPERS="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-result.beta"
V3RESULT="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3-source-only-result.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3_bundle.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir2.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v2-to-elf.alp"
LOWER_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow2.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/role3_resolution_fixture.py"
RESOLUTION_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/resolution_handoff_reference.py"
ALL_OP_SOURCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/source-custody-artifact.omg"
for REQUIRED in "$ENVELOPE" "$MODEL" "$COMMON" "$V3LOW" "$RESULT_HELPERS" \
  "$V3RESULT" "$PACKER" "$RESOLVER" "$LOWERER" "$BACKEND" \
  "$LOWER_FRAME" "$FIXTURE" "$RESOLUTION_REFERENCE" "$ALL_OP_SOURCE"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN3 layer 4: missing $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(date +%s)
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null

# Extract whole Beta procedures without depending on declaration order.  This
# lets OMGRFN3 reuse the frozen parser/model while physically replacing only
# the two call-sensitive procedures and excluding artifact readers from the
# source-only executable.
filter_procs() { # input output comma-separated exclusions
  python3 - "$1" "$2" "$3" <<'PY'
from pathlib import Path
import re
import sys

source = Path(sys.argv[1]).read_text(encoding="ascii")
excluded = set(filter(None, sys.argv[3].split(",")))
starts = list(re.finditer(r"(?m)^proc\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*\)\s*\{", source))
pieces = []
for match in starts:
    depth = 1
    cursor = match.end()
    while depth and cursor < len(source):
        depth += (source[cursor] == "{") - (source[cursor] == "}")
        cursor += 1
    if depth:
        raise SystemExit(f"unterminated procedure {match.group(1)}")
    if match.group(1) not in excluded:
        pieces.append(source[match.start():cursor].rstrip() + "\n")
Path(sys.argv[2]).write_text("\n".join(pieces), encoding="ascii")
PY
}

sed 's/omgrfn2_component/omgrfn3_component/g' "$MODEL" \
  | sed '/^proc main/,$d' > "$T/model.beta"
filter_procs "$COMMON" "$T/common-lowering.beta" \
  'src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_refinement_lowering_check,main'
filter_procs "$COMMON" "$T/common-source.beta" \
  'ckir_u32,ckir_row_word,ckir_row_byte,ckir_bparam_word,ckir_operand,src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_block_owner,src_lower_compare_final,src_refinement_lowering_check,main'
sed '/^proc v3_ckir_header_check/,$d' "$V3LOW" > "$T/v3-source-prefix.beta"
filter_procs "$RESULT_HELPERS" "$T/result-helpers.beta" \
  'src_eval_snapshot,src_eval_commit,src_eval_copy,src_eval_edge,src_refinement_source_result_check,main'

cp "$ENVELOPE" "$T/lowering.beta"
cat "$T/model.beta" "$T/common-lowering.beta" "$V3LOW" >> "$T/lowering.beta"

# CKIR and ELF component accessors are absent, not merely unused.
sed '/^proc omgrfn3_component_ckir_byte/,$d' "$ENVELOPE" > "$T/result.beta"
cat "$T/model.beta" "$T/common-source.beta" "$T/v3-source-prefix.beta" \
  "$T/result-helpers.beta" "$V3RESULT" >> "$T/result.beta"

python3 - "$T/result.beta" <<'PY'
from pathlib import Path
import re
import sys

text = Path(sys.argv[1]).read_text(encoding="ascii")
procedures = {}
spans = {}
for match in re.finditer(r"(?m)^proc\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*\)\s*\{", text):
    depth = 1
    cursor = match.end()
    while depth and cursor < len(text):
        depth += (text[cursor] == "{") - (text[cursor] == "}")
        cursor += 1
    procedures[match.group(1)] = text[match.end():cursor - 1]
    spans[match.group(1)] = (match.start(), cursor)
reachable = {"main"}
pending = ["main"]
while pending:
    body = procedures[pending.pop()]
    for called in re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(", body):
        if called in procedures and called not in reachable:
            reachable.add(called)
            pending.append(called)
forbidden = {
    "omgrfn3_component_ckir_byte", "omgrfn3_component_elf_byte",
    "refinement_ckir_byte", "refinement_elf_byte", "ckir_u32",
    "ckir_row_word", "ckir_row_byte", "ckir_operand",
    "v3_ckir_header_check", "src_lower_compare_final",
}
bad = sorted(reachable & forbidden)
if bad:
    raise SystemExit(f"source-only result reaches artifact readers: {bad}")
if "omgrfn3_component_ckir_byte" in text or "omgrfn3_component_elf_byte" in text:
    raise SystemExit("source-only executable physically contains artifact readers")
# Persist only the transitive source-result checker.  Composition helpers that
# are unreachable from main assign no evidence and needlessly consume Alpha's
# fixed seed hole.
ordered = sorted(reachable, key=lambda name: spans[name][0])
Path(sys.argv[1]).write_text(
    "\n\n".join(text[spans[name][0]:spans[name][1]].rstrip() for name in ordered) + "\n",
    encoding="ascii",
)
PY
COMPOSE_SECONDS=$(($(date +%s)-STARTED))

build_checker() {
  NAME=$1
  BUILD_STARTED=$(date +%s)
  PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/$NAME.beta")
  [ "$PROCEDURES" -le 128 ] || {
    echo "OMGRFN3 layer 4: $NAME exceeds 128 procedures ($PROCEDURES)" >&2
    exit 1
  }
  "$BC" < "$T/$NAME.beta" > "$T/$NAME.asm" || {
    echo "OMGRFN3 layer 4: $NAME Beta compilation failed" >&2
    return 1
  }
  "$ASM" < "$T/$NAME.asm" > "$T/$NAME.tape" || {
    echo "OMGRFN3 layer 4: $NAME Alpha assembly failed (asm $(wc -c < "$T/$NAME.asm" | tr -d ' ') bytes)" >&2
    return 1
  }
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME" >/dev/null 2>&1 || {
    echo "OMGRFN3 layer 4: $NAME stamping failed (asm $(wc -c < "$T/$NAME.asm" | tr -d ' ') bytes, tape $(wc -c < "$T/$NAME.tape" | tr -d ' ') bytes)" >&2
    return 1
  }
  echo "$(($(date +%s)-BUILD_STARTED))" > "$T/$NAME.build-seconds"
}
BUILD_PHASE_STARTED=$(date +%s)
build_checker lowering &
LOWERING_BUILD_PID=$!
build_checker result &
RESULT_BUILD_PID=$!
set +e
wait "$LOWERING_BUILD_PID"
LOWERING_BUILD_STATUS=$?
wait "$RESULT_BUILD_PID"
RESULT_BUILD_STATUS=$?
set -e
[ "$LOWERING_BUILD_STATUS" -eq 0 ] && [ "$RESULT_BUILD_STATUS" -eq 0 ] || exit 1
BUILD_WALL_SECONDS=$(($(date +%s)-BUILD_PHASE_STARTED))

PRODUCER_PHASE_STARTED=$(date +%s)
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend" >/dev/null
PRODUCER_SECONDS=$(($(date +%s)-PRODUCER_PHASE_STARTED))

run_expect() { # executable input status output label
  set +e
  "$1" < "$2" > "$4" 2> "$4.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$3" ] || {
    echo "OMGRFN3 layer 4: $5 returned $ACTUAL, expected $3" >&2
    sed -n '1,20p' "$4.stderr" >&2
    exit 1
  }
  [ "$3" -eq 0 ] || [ ! -s "$4" ] || {
    echo "OMGRFN3 layer 4: $5 published output on rejection" >&2
    exit 1
  }
}
observe() { # checker expected input label
  run_expect "$1" "$3" "$2" "$T/observe.out" "$4"
}
observe_both() {
  observe "$T/lowering" "$1" "$2" "$3 lowering"
  observe "$T/result" "$1" "$2" "$3 source result"
}
pack() { # omgcomp witness ckir elf result output
  python3 "$PACKER" "$1" "$2" "$3" "$4" --result "$5" > "$6"
}
pipeline() { # omgcomp prefix
  run_expect "$T/resolver" "$1" 0 "$T/$2.witness" "$2 resolver"
  python3 "$LOWER_FRAME" pack "$1" "$T/$2.witness" > "$T/$2.low"
  run_expect "$T/lowerer" "$T/$2.low" 0 "$T/$2.ckir" "$2 lowerer"
  run_expect "$T/backend" "$T/$2.ckir" 0 "$T/$2.elf" "$2 backend"
}

CHECK_PHASE_STARTED=$(date +%s)
python3 "$FIXTURE" build "$T/fixture"
CANONICAL="$T/fixture/valid.omgc"
pipeline "$CANONICAL" canonical
pack "$CANONICAL" "$T/canonical.witness" "$T/canonical.ckir" \
  "$T/canonical.elf" 70 "$T/canonical.rfn"
observe_both 0 "$T/canonical.rfn" "three-machine cross-source call DAG"

# Reuse the established complete operation corpus through CKIR2 and require an
# actual opcode-7 row.  This exercises the compact overlap-safe raw-layout Copy
# evaluator as part of the unchanged OMGRFN2 obligations inherited by V3.
python3 - "$RESOLUTION_REFERENCE" "$ALL_OP_SOURCE" "$T/all-ops.omgc" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, str(Path(sys.argv[1]).parent))
from resolution_handoff_reference import one_source
source = "module app;\n\n" + Path(sys.argv[2]).read_text(encoding="ascii")
Path(sys.argv[3]).write_bytes(one_source(source, module="app", owner="Probe"))
PY
pipeline "$T/all-ops.omgc" all-ops
python3 - "$T/all-ops.ckir" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
counts = struct.unpack_from("<12I", raw, 24)
strides = (24,20,16,36,20,32,20,40,4,44)
cursor = 72
bases = []
for count, stride in zip(counts, strides):
    bases.append(cursor); cursor += count * stride
opcodes = {raw[bases[7] + row * 40 + 12] for row in range(counts[7])}
if 7 not in opcodes:
    raise SystemExit("all-operation CKIR2 lacks Copy opcode 7")
PY
pack "$T/all-ops.omgc" "$T/all-ops.witness" "$T/all-ops.ckir" \
  "$T/all-ops.elf" 70 "$T/all-ops.rfn"
observe_both 0 "$T/all-ops.rfn" "CKIR2 all-operation Copy coverage"

# A fixed-width source result mutation preserves all role-3 spans and creates
# a second valid source/witness/CKIR2/ELF relation with result 71.
python3 - "$T/changed.omgc" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, "source/on-ramp/omega-bootstrap/gates")
from role3_resolution_fixture import ROOT_SOURCE, SECOND_SOURCE, encode
Path(sys.argv[1]).write_bytes(encode(ROOT_SOURCE.replace("local(68)", "local(69)"), SECOND_SOURCE))
PY
pipeline "$T/changed.omgc" changed
pack "$T/changed.omgc" "$T/changed.witness" "$T/changed.ckir" \
  "$T/changed.elf" 71 "$T/changed.rfn"
observe_both 0 "$T/changed.rfn" "second valid call result"
pack "$T/changed.omgc" "$T/changed.witness" "$T/changed.ckir" \
  "$T/changed.elf" 70 "$T/wrong-result.rfn"
observe "$T/lowering" 0 "$T/wrong-result.rfn" "lowering result-claim independence"
observe "$T/result" 251 "$T/wrong-result.rfn" "source call result mismatch"
pack "$T/changed.omgc" "$T/changed.witness" "$T/canonical.ckir" \
  "$T/canonical.elf" 70 "$T/result-cross-pair.rfn"
observe_both 251 "$T/result-cross-pair.rfn" "source/CKIR/result cross-pair"

# Exact target, source span, and once-only binding consumption controls.
python3 - "$T/canonical.witness" "$T/binding-target.witness" \
  "$T/binding-span.witness" "$T/binding-extra.witness" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
counts = struct.unpack_from("<11I", raw, 20)
strides = (36,48,28,28,24,24,24,40,24,40,24)
bases=[]; cursor=72
for count,stride in zip(counts,strides):
    bases.append(cursor); cursor += count*stride
role3 = [bases[2]+i*28 for i in range(counts[2]) if raw[bases[2]+i*28+8] == 3]
if len(role3) != 2:
    raise SystemExit("canonical role-3 count drifted")
changed=bytearray(raw); struct.pack_into("<I",changed,role3[0]+20,4)
Path(sys.argv[2]).write_bytes(changed)
changed=bytearray(raw); start=struct.unpack_from("<I",changed,role3[0]+12)[0]; struct.pack_into("<I",changed,role3[0]+12,start+1)
Path(sys.argv[3]).write_bytes(changed)
changed=bytearray(raw); row=bases[2]; changed[row+8]=3; changed[row+9]=2; struct.pack_into("<II",changed,row+20,1,0xffffffff)
Path(sys.argv[4]).write_bytes(changed)
PY
for CASE in binding-target binding-span binding-extra; do
  pack "$CANONICAL" "$T/$CASE.witness" "$T/canonical.ckir" \
    "$T/canonical.elf" 70 "$T/$CASE.rfn"
  observe_both 251 "$T/$CASE.rfn" "$CASE role-3 control"
done

# CKIR callee, receiver/argument order, and call-result controls are local to
# the lowering checker; the source-only executable remains artifact-oblivious.
python3 - "$T/canonical.ckir" "$T/ckir-mutations" <<'PY'
from pathlib import Path
import struct
import sys
raw=Path(sys.argv[1]).read_bytes(); out=Path(sys.argv[2]); out.mkdir()
counts=struct.unpack_from("<12I",raw,24); strides=(24,20,16,36,20,32,20,40,4,44)
bases=[]; cursor=72
for count,stride in zip(counts,strides): bases.append(cursor); cursor += count*stride
calls=[bases[7]+i*40 for i in range(counts[7]) if raw[bases[7]+i*40+12] == 10]
if len(calls) != 2: raise SystemExit("canonical call count drifted")
def word(name,at,value):
    changed=bytearray(raw); struct.pack_into("<I",changed,at,value); (out/f"{name}.ckir").write_bytes(changed)
word("callee",calls[0]+32,3)
word("result-type",calls[0]+20,2)
operand_start=struct.unpack_from("<I",raw,calls[0]+24)[0]
first=struct.unpack_from("<I",raw,bases[8]+operand_start*4)[0]
second=struct.unpack_from("<I",raw,bases[8]+(operand_start+1)*4)[0]
changed=bytearray(raw); struct.pack_into("<II",changed,bases[8]+operand_start*4,second,first)
(out/"order.ckir").write_bytes(changed)
PY
for CASE in callee result-type order; do
  pack "$CANONICAL" "$T/canonical.witness" "$T/ckir-mutations/$CASE.ckir" \
    "$T/canonical.elf" 70 "$T/$CASE.rfn"
  observe "$T/lowering" 251 "$T/$CASE.rfn" "$CASE CKIR2 call relation"
  observe "$T/result" 0 "$T/$CASE.rfn" "$CASE artifact independence"
done

# Direct and unreachable cycles are coherent source/witness pairs.  They are
# rejected by the complete source-derived graph before artifact comparison.
python3 - "$T/direct-cycle.omgc" "$T/unreachable-cycle.omgc" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, "source/on-ramp/omega-bootstrap/gates")
from role3_resolution_fixture import ROOT_SOURCE, SECOND_SOURCE, encode
Path(sys.argv[1]).write_bytes(encode(ROOT_SOURCE.replace("self.local(68)", "self.run()"), SECOND_SOURCE))
Path(sys.argv[2]).write_bytes(encode(ROOT_SOURCE, SECOND_SOURCE.replace("    7\n", "    self.decoy()\n")))
PY
for CASE in direct-cycle unreachable-cycle; do
  run_expect "$T/resolver" "$T/$CASE.omgc" 0 "$T/$CASE.witness" "$CASE resolver"
  pack "$T/$CASE.omgc" "$T/$CASE.witness" "$T/canonical.ckir" \
    "$T/canonical.elf" 70 "$T/$CASE.rfn"
  observe_both 251 "$T/$CASE.rfn" "$CASE whole source call DAG"
done

# The source evaluator has a published 16-frame evidence-storage ceiling.
# A coherent 17-machine chain remains a valid lowering relation but exhausts
# that evaluator locally with status 252.
python3 - "$T/deep.omgc" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, "source/on-ramp/omega-bootstrap/gates")
from role3_resolution_fixture import encode
parts = ["module app;", "data Probe {}"]
parts.append("machine Probe::run(&mut self) -> u8 { self.m1() }")
for index in range(1, 16):
    parts.append(f"machine Probe::m{index}(&mut self) -> u8 {{ self.m{index+1}() }}")
parts.append("machine Probe::m16(&mut self) -> u8 { 70 }")
Path(sys.argv[1]).write_bytes(encode("\n".join(parts) + "\n", "module app;\n"))
PY
pipeline "$T/deep.omgc" deep
pack "$T/deep.omgc" "$T/deep.witness" "$T/deep.ckir" \
  "$T/deep.elf" 70 "$T/deep.rfn"
observe "$T/lowering" 0 "$T/deep.rfn" "17-machine lowering relation"
observe "$T/result" 252 "$T/deep.rfn" "16-frame source-evaluation resource bound"

# The source executable accepts opaque artifact components because it cannot
# read them.  The lowering executable rejects the same malformed CKIR header.
printf opaque-ckir > "$T/opaque.ckir"
printf opaque-elf > "$T/opaque.elf"
pack "$CANONICAL" "$T/canonical.witness" "$T/opaque.ckir" \
  "$T/opaque.elf" 70 "$T/opaque.rfn"
observe "$T/result" 0 "$T/opaque.rfn" "source-only physical artifact independence"
observe "$T/lowering" 251 "$T/opaque.rfn" "malformed CKIR2 header"

# Representative malformed/resource teeth remain phase-local.
python3 - "$T/canonical.witness" "$T/bad.witness" "$T/over.witness" \
  "$T/canonical.ckir" "$T/over.ckir" <<'PY'
from pathlib import Path
import struct
import sys
raw=bytearray(Path(sys.argv[1]).read_bytes()); raw[0]^=1; Path(sys.argv[2]).write_bytes(raw)
raw=bytearray(Path(sys.argv[1]).read_bytes()); struct.pack_into("<I",raw,36,2049); Path(sys.argv[3]).write_bytes(raw)
raw=bytearray(Path(sys.argv[4]).read_bytes()); struct.pack_into("<I",raw,52,32769); Path(sys.argv[5]).write_bytes(raw)
PY
pack "$CANONICAL" "$T/bad.witness" "$T/canonical.ckir" "$T/canonical.elf" 70 "$T/bad.rfn"
pack "$CANONICAL" "$T/over.witness" "$T/canonical.ckir" "$T/canonical.elf" 70 "$T/over-witness.rfn"
pack "$CANONICAL" "$T/canonical.witness" "$T/over.ckir" "$T/canonical.elf" 70 "$T/over-ckir.rfn"
observe_both 251 "$T/bad.rfn" "malformed witness"
observe_both 252 "$T/over-witness.rfn" "witness resource"
observe "$T/lowering" 252 "$T/over-ckir.rfn" "CKIR2 resource"
observe "$T/result" 0 "$T/over-ckir.rfn" "source result CKIR2 resource independence"

ELAPSED=$(($(date +%s)-STARTED))
LOWERING_PROCS=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/lowering.beta")
RESULT_PROCS=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/result.beta")
LOWERING_BUILD_SECONDS=$(cat "$T/lowering.build-seconds")
RESULT_BUILD_SECONDS=$(cat "$T/result.build-seconds")
LOWERING_ASM_BYTES=$(wc -c < "$T/lowering.asm" | tr -d ' ')
RESULT_ASM_BYTES=$(wc -c < "$T/result.asm" | tr -d ' ')
LOWERING_TAPE_BYTES=$(wc -c < "$T/lowering.tape" | tr -d ' ')
RESULT_TAPE_BYTES=$(wc -c < "$T/result.tape" | tr -d ' ')
CHECK_SECONDS=$(($(date +%s)-CHECK_PHASE_STARTED))
echo "OMGRFN3 layer 4: role-3 bodies -> CKIR2 and artifact-free distinct-frame call result passed below Delta (${LOWERING_PROCS}/${RESULT_PROCS} procedures; asm ${LOWERING_ASM_BYTES}/${RESULT_ASM_BYTES} bytes, tape ${LOWERING_TAPE_BYTES}/${RESULT_TAPE_BYTES} bytes; compose ${COMPOSE_SECONDS}s, parallel builds ${BUILD_WALL_SECONDS}s wall and ${LOWERING_BUILD_SECONDS}s/${RESULT_BUILD_SECONDS}s each, producers ${PRODUCER_SECONDS}s, controls ${CHECK_SECONDS}s, total ${ELAPSED}s)"
