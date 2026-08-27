#!/usr/bin/env sh
# Focused same-exact-frame composition of all independent OMGRFN3 duties.
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
  *) echo "OMGRFN3 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN3 same-frame composite: skipped ($TOOL absent)"
    exit 0
  }
done

R="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT"
ENVELOPE="$R/omgrfn3-component-envelope.beta"
L1="$R/omgrfn3-frame-omgcomp-custody.beta"
L2="$R/omgrfn3-source-witness-independent.beta"
L3="$R/omgrfn3-witness-ckir2-tables.beta"
MODEL="$R/omgrfn2-resolved-body-model.beta"
COMMON="$R/ckir-refinement-source-lowering.beta"
V3LOW="$R/omgrfn3-resolved-body-lowering.beta"
RESULT_HELPERS="$R/ckir-refinement-source-result.beta"
V3RESULT="$R/omgrfn3-source-only-result.beta"
ARTIFACT="$R/ckir2-refinement-artifact.beta"
ELF_CHECKER="$R/ckir2-refinement-elf.beta"
PACKER="$R/omgrfn3_bundle.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir2.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v2-to-elf.alp"
LOWER_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow2.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/role3_resolution_fixture.py"
for REQUIRED in "$ENVELOPE" "$L1" "$L2" "$L3" "$MODEL" "$COMMON" \
  "$V3LOW" "$RESULT_HELPERS" "$V3RESULT" "$ARTIFACT" "$ELF_CHECKER" \
  "$PACKER" "$RESOLVER" "$LOWERER" "$BACKEND" "$LOWER_FRAME" "$FIXTURE"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN3 same-frame composite: missing $REQUIRED" >&2
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

filter_procs() { # input output comma-separated exclusions
  python3 - "$1" "$2" "$3" <<'PY'
from pathlib import Path
import re, sys
source = Path(sys.argv[1]).read_text(encoding="ascii")
excluded = set(filter(None, sys.argv[3].split(",")))
starts = list(re.finditer(r"(?m)^proc\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*\)\s*\{", source))
pieces = []
for match in starts:
    depth, cursor = 1, match.end()
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

# Responsibilities 1--3 stay byte-for-byte independent checkers.
cp "$L1" "$T/layer1.beta"
printf '\nproc main() { return omgrfn3_layer1_check() }\n' >> "$T/layer1.beta"
cp "$L2" "$T/layer2.beta"
printf '\nproc main() { return omgrfn3_l2_check() }\n' >> "$T/layer2.beta"
cp "$L3" "$T/layer3.beta"

# Responsibility 4 deliberately remains two executables.  The source-result
# executable contains no CKIR2 or ELF reader, while lowering owns the exact
# source-body -> CKIR2 relation.
sed 's/omgrfn2_component/omgrfn3_component/g' "$MODEL" \
  | sed '/^proc main/,$d' > "$T/model.beta"
filter_procs "$COMMON" "$T/common-lowering.beta" \
  'src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_refinement_lowering_check,main'
filter_procs "$COMMON" "$T/common-source.beta" \
  'ckir_u32,ckir_row_word,ckir_row_byte,ckir_bparam_word,ckir_operand,src_low_decode_validated_ckir,src_low_scalar_assignable,src_low_emit,src_low_postfix,src_low_block_owner,src_lower_compare_final,src_refinement_lowering_check,main'
sed '/^proc v3_ckir_header_check/,$d' "$V3LOW" > "$T/v3-source-prefix.beta"
filter_procs "$RESULT_HELPERS" "$T/result-helpers.beta" \
  'src_eval_snapshot,src_eval_commit,src_eval_copy,src_eval_edge,src_refinement_source_result_check,main'
cp "$ENVELOPE" "$T/layer4-lowering.beta"
cat "$T/model.beta" "$T/common-lowering.beta" "$V3LOW" >> "$T/layer4-lowering.beta"
sed '/^proc omgrfn3_component_ckir_byte/,$d' "$ENVELOPE" > "$T/layer4-result.beta"
cat "$T/model.beta" "$T/common-source.beta" "$T/v3-source-prefix.beta" \
  "$T/result-helpers.beta" "$V3RESULT" >> "$T/layer4-result.beta"

# Keep only the source-result main closure and prove artifact readers are
# physically absent from that persisted program.
python3 - "$T/layer4-result.beta" <<'PY'
from pathlib import Path
import re, sys
path = Path(sys.argv[1]); text = path.read_text(encoding="ascii")
procedures, spans = {}, {}
for match in re.finditer(r"(?m)^proc\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*\)\s*\{", text):
    depth, cursor = 1, match.end()
    while depth and cursor < len(text):
        depth += (text[cursor] == "{") - (text[cursor] == "}")
        cursor += 1
    procedures[match.group(1)] = text[match.end():cursor - 1]
    spans[match.group(1)] = (match.start(), cursor)
reachable, pending = {"main"}, ["main"]
while pending:
    for called in re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(", procedures[pending.pop()]):
        if called in procedures and called not in reachable:
            reachable.add(called); pending.append(called)
forbidden = {"omgrfn3_component_ckir_byte", "omgrfn3_component_elf_byte",
             "refinement_ckir_byte", "refinement_elf_byte", "ckir_u32",
             "ckir_row_word", "ckir_row_byte", "ckir_operand",
             "v3_ckir_header_check", "src_lower_compare_final"}
if reachable & forbidden:
    raise SystemExit(f"source-result reaches artifact readers: {sorted(reachable & forbidden)}")
if "omgrfn3_component_ckir_byte" in text or "omgrfn3_component_elf_byte" in text:
    raise SystemExit("source-result physically contains artifact readers")
ordered = sorted(reachable, key=lambda name: spans[name][0])
path.write_text("\n\n".join(text[spans[n][0]:spans[n][1]].rstrip() for n in ordered) + "\n", encoding="ascii")
PY

# Responsibility 5 remains independently observable as complete CKIR/result
# validation and as CKIR/result followed by exact ELF validation.
cp "$ENVELOPE" "$T/layer5-artifact.beta"
sed '/^proc main()/,$d' "$ARTIFACT" >> "$T/layer5-artifact.beta"
printf '%s\n' '' 'proc main() {' \
  '    let status = omgrfn3_component_read()' \
  '    state envelope { to done when (status != 0)  status = ckir_refinement_artifact_check()  to done }' \
  '    state done { return status }' '}' >> "$T/layer5-artifact.beta"
cp "$ENVELOPE" "$T/layer5-elf.beta"
sed '/^proc main()/,$d' "$ARTIFACT" >> "$T/layer5-elf.beta"
sed '/^proc main()/,$d' "$ELF_CHECKER" >> "$T/layer5-elf.beta"
printf '%s\n' '' 'proc main() {' \
  '    let status = omgrfn3_component_read()' \
  '    state envelope { to done when (status != 0)  status = ckir_refinement_artifact_check()  to artifact }' \
  '    state artifact { to done when (status != 0)  status = ckir2_refinement_elf_check()  to done }' \
  '    state done { return status }' '}' >> "$T/layer5-elf.beta"

CHECKERS='layer1 layer2 layer3 layer4-lowering layer4-result layer5-artifact layer5-elf'
for NAME in $CHECKERS; do
  PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/$NAME.beta")
  [ "$PROCEDURES" -le 128 ] || {
    echo "OMGRFN3 same-frame composite: $NAME exceeds 128 procedures ($PROCEDURES)" >&2
    exit 1
  }
  echo "$PROCEDURES" > "$T/$NAME.procedures"
done

build_checker() {
  NAME=$1
  BUILD_STARTED=$(date +%s)
  "$BC" < "$T/$NAME.beta" > "$T/$NAME.asm"
  "$ASM" < "$T/$NAME.asm" > "$T/$NAME.tape"
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME" >/dev/null 2>&1
  echo "$(($(date +%s)-BUILD_STARTED))" > "$T/$NAME.build-seconds"
}
BUILD_STARTED=$(date +%s)
PIDS=''
for NAME in $CHECKERS; do
  build_checker "$NAME" &
  PIDS="$PIDS $!"
done
for PID in $PIDS; do wait "$PID"; done
BUILD_WALL=$(($(date +%s)-BUILD_STARTED))

PRODUCER_STARTED=$(date +%s)
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend" >/dev/null
python3 "$FIXTURE" build "$T/fixture"
CANONICAL_OMGCOMP="$T/fixture/valid.omgc"
"$T/resolver" < "$CANONICAL_OMGCOMP" > "$T/canonical.witness"
python3 "$LOWER_FRAME" pack "$CANONICAL_OMGCOMP" "$T/canonical.witness" > "$T/canonical.low"
"$T/lowerer" < "$T/canonical.low" > "$T/canonical.ckir"
"$T/backend" < "$T/canonical.ckir" > "$T/canonical.elf"
python3 "$PACKER" "$CANONICAL_OMGCOMP" "$T/canonical.witness" \
  "$T/canonical.ckir" "$T/canonical.elf" --result 70 > "$T/canonical.rfn"
PRODUCER_SECONDS=$(($(date +%s)-PRODUCER_STARTED))

observe() { # executable expected frame label
  EXE=$1 EXPECTED=$2 INPUT=$3 LABEL=$4
  set +e
  "$T/$EXE" < "$INPUT" > "$T/$EXE.out" 2> "$T/$EXE.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN3 same-frame composite: $LABEL/$EXE returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/$EXE.stderr" >&2
    exit 1
  }
  [ ! -s "$T/$EXE.out" ] || {
    echo "OMGRFN3 same-frame composite: $LABEL/$EXE published stdout" >&2
    exit 1
  }
}

observe_all() { # expected frame label
  for NAME in $CHECKERS; do observe "$NAME" "$1" "$2" "$3"; done
}

CONTROL_STARTED=$(date +%s)
# The single positive frame is fed unchanged to every executable.
observe_all 0 "$T/canonical.rfn" canonical

# A second valid source/witness pair changes only the call constant and thus
# keeps the declaration/table shape stable.  Cross-pairing it with canonical
# CKIR2/result/ELF is accepted by the custody, source/witness, table, and
# artifact-only phases, while both source-body responsibilities reject it.
python3 - "$T/changed.omgc" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, "source/on-ramp/omega-bootstrap/gates")
from role3_resolution_fixture import ROOT_SOURCE, SECOND_SOURCE, encode
Path(sys.argv[1]).write_bytes(encode(ROOT_SOURCE.replace("local(68)", "local(69)"), SECOND_SOURCE))
PY
"$T/resolver" < "$T/changed.omgc" > "$T/changed.witness"
python3 "$PACKER" "$T/changed.omgc" "$T/changed.witness" \
  "$T/canonical.ckir" "$T/canonical.elf" --result 70 > "$T/source-artifact-crosspair.rfn"
for NAME in layer1 layer2 layer3 layer5-artifact layer5-elf; do
  observe "$NAME" 0 "$T/source-artifact-crosspair.rfn" source-artifact-crosspair-opaque
done
observe layer4-lowering 251 "$T/source-artifact-crosspair.rfn" source-artifact-crosspair-lowering
observe layer4-result 251 "$T/source-artifact-crosspair.rfn" source-artifact-crosspair-result

# Whole opaque artifact components are still a valid input to the source-only
# duties.  The source-result executable's acceptance is backed by the earlier
# composition proof that it physically contains neither artifact accessor.
printf opaque-ckir2 > "$T/opaque.ckir"
printf opaque-elf > "$T/opaque.elf"
python3 "$PACKER" "$CANONICAL_OMGCOMP" "$T/canonical.witness" \
  "$T/opaque.ckir" "$T/opaque.elf" --result 70 > "$T/opaque-artifacts.rfn"
for NAME in layer1 layer2 layer4-result; do
  observe "$NAME" 0 "$T/opaque-artifacts.rfn" whole-artifact-opacity
done
for NAME in layer3 layer4-lowering layer5-artifact layer5-elf; do
  observe "$NAME" 251 "$T/opaque-artifacts.rfn" malformed-artifact-owned
done

# Derive byte-local controls from that exact frame.  Witness target corruption
# is opaque to layer 1, the table-only layer 3, and layer 5, but rejected by
# the source/witness and source-body duties that own role-3 binding identity.
# A call-callee mutation preserves tables and is opaque to layers 1--3 and the
# source-only evaluator.  An ELF-only mutation is opaque to every duty except
# the final CKIR2 -> exact-ELF checker.
python3 - "$T/canonical.rfn" "$T" <<'PY'
from pathlib import Path
import struct, sys
raw = Path(sys.argv[1]).read_bytes(); out = Path(sys.argv[2])
magic, version, flags, omg_len, wit_len, ckir_len, elf_len, result, exit_status = struct.unpack_from("<8s8I", raw)
assert magic == b"OMGRFN3\0" and version == 3 and flags == 1 and result == 70 and exit_status == 70
wit_at = 40 + omg_len; ckir_at = wit_at + wit_len; elf_at = ckir_at + ckir_len

witness = raw[wit_at:ckir_at]
counts = struct.unpack_from("<11I", witness, 20)
strides = (36,48,28,28,24,24,24,40,24,40,24)
bases=[]; cursor=72
for count,stride in zip(counts,strides): bases.append(cursor); cursor += count*stride
role3=[bases[2]+i*28 for i in range(counts[2]) if witness[bases[2]+i*28+8] == 3]
assert len(role3) == 2
changed=bytearray(raw); struct.pack_into("<I",changed,wit_at+role3[0]+20,4)
out.joinpath("witness-target.rfn").write_bytes(changed)

ckir = raw[ckir_at:elf_at]
counts = struct.unpack_from("<12I", ckir, 24)
strides=(24,20,16,36,20,32,20,40,4,44)
bases=[]; cursor=72
for count,stride in zip(counts,strides): bases.append(cursor); cursor += count*stride
calls=[bases[7]+i*40 for i in range(counts[7]) if ckir[bases[7]+i*40+12] == 10]
assert len(calls) == 2
changed=bytearray(raw); struct.pack_into("<I",changed,ckir_at+calls[0]+32,3)
out.joinpath("ckir-callee.rfn").write_bytes(changed)

changed=bytearray(raw); changed[elf_at + elf_len - 1] ^= 1
out.joinpath("elf-byte.rfn").write_bytes(changed)
PY

observe layer1 0 "$T/witness-target.rfn" witness-target-opaque
observe layer3 0 "$T/witness-target.rfn" witness-target-opaque
for NAME in layer2 layer4-lowering layer4-result; do
  observe "$NAME" 251 "$T/witness-target.rfn" witness-target-owned
done
observe layer5-artifact 0 "$T/witness-target.rfn" witness-target-opaque
observe layer5-elf 0 "$T/witness-target.rfn" witness-target-opaque

for NAME in layer1 layer2 layer3 layer4-result; do
  observe "$NAME" 0 "$T/ckir-callee.rfn" ckir-body-opaque
done
observe layer4-lowering 251 "$T/ckir-callee.rfn" ckir-body-source-join
observe layer5-artifact 251 "$T/ckir-callee.rfn" ckir-body-artifact
observe layer5-elf 251 "$T/ckir-callee.rfn" ckir-body-artifact-before-elf

for NAME in layer1 layer2 layer3 layer4-lowering layer4-result layer5-artifact; do
  observe "$NAME" 0 "$T/elf-byte.rfn" elf-opaque
done
observe layer5-elf 251 "$T/elf-byte.rfn" elf-owned
CONTROL_SECONDS=$(($(date +%s)-CONTROL_STARTED))

PROC_REPORT=''
BUILD_REPORT=''
for NAME in $CHECKERS; do
  PROC_REPORT="${PROC_REPORT}${NAME}=$(cat "$T/$NAME.procedures"),"
  BUILD_REPORT="${BUILD_REPORT}${NAME}=$(cat "$T/$NAME.build-seconds")s,"
done
FRAME_BYTES=$(wc -c < "$T/canonical.rfn" | tr -d ' ')
TOTAL=$(($(date +%s)-STARTED))
echo "OMGRFN3 same-frame composite: all five responsibilities accepted one ${FRAME_BYTES}-byte canonical role-3 frame; witness/CKIR/ELF isolation matrix passed (procedures ${PROC_REPORT%,}; builds ${BUILD_REPORT%,}; build wall ${BUILD_WALL}s, producers ${PRODUCER_SECONDS}s, controls ${CONTROL_SECONDS}s, total ${TOTAL}s)"
