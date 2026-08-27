#!/usr/bin/env sh
# Focused persisted-Beta OMGRFN2 layer-4 body lowering and source-only result.
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
  *) echo "OMGRFN2 layer 4: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN2 layer 4: skipped ($TOOL absent)"
    exit 0
  }
done

ENVELOPE="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2-component-envelope.beta"
MODEL="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2-resolved-body-model.beta"
LOWERING="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-lowering.beta"
LOWERING_MAIN="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2-resolved-body-lowering.beta"
RESULT="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-result.beta"
RESULT_MAIN="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2-source-only-result.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2_bundle.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-to-elf.alp"
LOWER_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
RESOLUTION_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/resolution_handoff_reference.py"
ALL_OP_SOURCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/source-custody-artifact.omg"
for REQUIRED in "$ENVELOPE" "$MODEL" "$LOWERING" "$LOWERING_MAIN" \
  "$RESULT" "$RESULT_MAIN" "$PACKER" "$RESOLVER" "$LOWERER" "$BACKEND" \
  "$LOWER_FRAME" "$FIXTURE" "$RESOLUTION_REFERENCE" "$ALL_OP_SOURCE"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN2 layer 4: missing $REQUIRED" >&2
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

compose_base() {
  OUT=$1
  cp "$ENVELOPE" "$OUT"
  sed '/^proc main()/,$d' "$MODEL" >> "$OUT"
}

compose_source_base() {
  OUT=$1
  sed '/^proc omgrfn2_component_ckir_byte()/,$d' "$ENVELOPE" > "$OUT"
  sed '/^proc main()/,$d' "$MODEL" >> "$OUT"
}

compose_base "$T/lowering.beta"
sed '/^proc main()/,$d' "$LOWERING" >> "$T/lowering.beta"
cat "$LOWERING_MAIN" >> "$T/lowering.beta"

compose_source_base "$T/result.beta"
# The source-only composition selects only source-body reconstruction.  It
# omits the CKIR-reading prefix, the CKIR comparison suffix, and the wrapper
# that joins those two responsibilities.
sed -n '/^proc src_lower_op(/,/^proc src_lower_compare_final()/p' "$LOWERING" | sed '$d' >> "$T/result.beta"
sed -n '/^proc src_reconstruct_lowering_check()/,/^proc src_refinement_lowering_check()/p' "$LOWERING" | sed '$d' >> "$T/result.beta"
sed '/^proc main()/,$d' "$RESULT" >> "$T/result.beta"
cat "$RESULT_MAIN" >> "$T/result.beta"

python3 - "$T/result.beta" <<'PY'
from pathlib import Path
import re
import sys

text = Path(sys.argv[1]).read_text(encoding="ascii")
procedures = {}
for match in re.finditer(r"(?m)^proc\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^)]*\)\s*\{", text):
    name = match.group(1)
    depth = 1
    cursor = match.end()
    while depth and cursor < len(text):
        depth += (text[cursor] == "{") - (text[cursor] == "}")
        cursor += 1
    if depth:
        raise SystemExit(f"unterminated procedure {name}")
    procedures[name] = text[match.end():cursor - 1]

reachable = {"main"}
pending = ["main"]
while pending:
    name = pending.pop()
    body = procedures[name]
    for called in re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(", body):
        if called in procedures and called not in reachable:
            reachable.add(called)
            pending.append(called)

forbidden = {
    "omgrfn2_component_ckir_byte",
    "omgrfn2_component_elf_byte",
    "refinement_ckir_byte",
    "refinement_elf_byte",
    "src_low_decode_validated_ckir",
    "src_lower_compare_final",
}
bad = sorted(reachable & forbidden)
if bad:
    raise SystemExit(f"source-only result reaches artifact readers: {bad}")
PY

build_checker() {
  NAME=$1
  PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/$NAME.beta")
  [ "$PROCEDURES" -le 128 ] || {
    echo "OMGRFN2 layer 4: $NAME exceeds 128 procedures ($PROCEDURES)" >&2
    exit 1
  }
  "$BC" < "$T/$NAME.beta" > "$T/$NAME.asm"
  "$ASM" < "$T/$NAME.asm" > "$T/$NAME.tape"
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME" >/dev/null 2>&1
}
build_checker lowering
build_checker result

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend" >/dev/null

python3 "$FIXTURE" build "$T/canonical"
CANONICAL_COMP="$T/canonical/compilation-envelope.bin"
"$T/resolver" < "$CANONICAL_COMP" > "$T/canonical.witness"
python3 "$LOWER_FRAME" pack "$CANONICAL_COMP" "$T/canonical.witness" > "$T/canonical.low"
"$T/lowerer" < "$T/canonical.low" > "$T/canonical.ckir"
"$T/backend" < "$T/canonical.ckir" > "$T/canonical.elf"

# A fixed-width body-only source change is a second valid compilation.  Its
# resolution witness must be byte-identical because OMGRSW1 deliberately
# carries no body operations.
python3 - "$CANONICAL_COMP" "$T/result-71.comp" <<'PY'
from pathlib import Path
import sys

contents = Path(sys.argv[1]).read_bytes()
needle = b"self.pair.first = 70;"
if contents.count(needle) != 1:
    raise SystemExit("canonical body mutation site is not unique")
Path(sys.argv[2]).write_bytes(contents.replace(needle, b"self.pair.first = 71;"))
PY
"$T/resolver" < "$T/result-71.comp" > "$T/result-71.witness"
cmp "$T/canonical.witness" "$T/result-71.witness" >/dev/null
python3 "$LOWER_FRAME" pack "$T/result-71.comp" "$T/result-71.witness" > "$T/result-71.low"
"$T/lowerer" < "$T/result-71.low" > "$T/result-71.ckir"
"$T/backend" < "$T/result-71.ckir" > "$T/result-71.elf"

# Reuse the complete one-unit source tranche through the modular OMGCOMP path.
# It supplies named states, branches, edge arguments, Add, Less, Index, Copy,
# and both operation and terminator families without duplicating their corpus.
# A fixed-width body-only mutation then changes one u32/u32 Less to u8/u32;
# the resolution witness remains identical, while source lowering must reject.
python3 - "$RESOLUTION_REFERENCE" "$ALL_OP_SOURCE" \
  "$T/all-ops.comp" "$T/mixed-less.comp" "$T/literal-range.comp" \
  "$T/literal-overflow.comp" <<'PY'
from pathlib import Path
import sys

sys.path.insert(0, str(Path(sys.argv[1]).parent))
from resolution_handoff_reference import one_source

source = "module app;\n\n" + Path(sys.argv[2]).read_text(encoding="ascii")
needle = "transition self.index < self.length"
replacement = "transition self.after < self.length"
if source.count(needle) != 1 or len(needle) != len(replacement):
    raise SystemExit("all-operation mixed-Less mutation site drifted")
Path(sys.argv[3]).write_bytes(one_source(source, module="app", owner="Probe"))
Path(sys.argv[4]).write_bytes(one_source(
    source.replace(needle, replacement), module="app", owner="Probe",
))
range_needle = "transition self.before < 8"
range_valid = "transition self.before < 100"
range_invalid = "transition self.before < 300"
if source.count(range_needle) != 1 or len(range_valid) != len(range_invalid):
    raise SystemExit("all-operation literal-range mutation site drifted")
range_source = source.replace(range_needle, range_valid)
Path(sys.argv[5]).write_bytes(one_source(
    range_source, module="app", owner="Probe",
))
Path(sys.argv[6]).write_bytes(one_source(
    range_source.replace(range_valid, range_invalid), module="app", owner="Probe",
))
PY
"$T/resolver" < "$T/all-ops.comp" > "$T/all-ops.witness"
"$T/resolver" < "$T/mixed-less.comp" > "$T/mixed-less.witness"
cmp "$T/all-ops.witness" "$T/mixed-less.witness" >/dev/null
python3 "$LOWER_FRAME" pack "$T/all-ops.comp" "$T/all-ops.witness" > "$T/all-ops.low"
"$T/lowerer" < "$T/all-ops.low" > "$T/all-ops.ckir"
"$T/backend" < "$T/all-ops.ckir" > "$T/all-ops.elf"
"$T/resolver" < "$T/literal-range.comp" > "$T/literal-range.witness"
"$T/resolver" < "$T/literal-overflow.comp" > "$T/literal-overflow.witness"
cmp "$T/literal-range.witness" "$T/literal-overflow.witness" >/dev/null
python3 "$LOWER_FRAME" pack "$T/literal-range.comp" \
  "$T/literal-range.witness" > "$T/literal-range.low"
"$T/lowerer" < "$T/literal-range.low" > "$T/literal-range.ckir"
"$T/backend" < "$T/literal-range.ckir" > "$T/literal-range.elf"

pack() { # omgcomp witness ckir elf result output
  python3 "$PACKER" "$1" "$2" "$3" "$4" --result "$5" > "$6"
}
observe() { # checker expected input label
  CHECKER=$1
  EXPECTED=$2
  INPUT=$3
  LABEL=$4
  set +e
  "$CHECKER" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN2 layer 4: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN2 layer 4: $LABEL published stdout" >&2
    exit 1
  }
}
observe_both() { # expected input label
  observe "$T/lowering" "$1" "$2" "$3 lowering"
  observe "$T/result" "$1" "$2" "$3 source result"
}

pack "$CANONICAL_COMP" "$T/canonical.witness" "$T/canonical.ckir" \
  "$T/canonical.elf" 70 "$T/canonical.rfn"
pack "$T/result-71.comp" "$T/result-71.witness" "$T/result-71.ckir" \
  "$T/result-71.elf" 71 "$T/result-71.rfn"
pack "$T/all-ops.comp" "$T/all-ops.witness" "$T/all-ops.ckir" \
  "$T/all-ops.elf" 70 "$T/all-ops.rfn"
pack "$T/mixed-less.comp" "$T/all-ops.witness" "$T/all-ops.ckir" \
  "$T/all-ops.elf" 70 "$T/mixed-less.rfn"
pack "$T/literal-range.comp" "$T/literal-range.witness" \
  "$T/literal-range.ckir" "$T/literal-range.elf" 70 "$T/literal-range.rfn"
pack "$T/literal-overflow.comp" "$T/literal-range.witness" \
  "$T/literal-range.ckir" "$T/literal-range.elf" 70 "$T/literal-overflow.rfn"
observe_both 0 "$T/canonical.rfn" "canonical cross-source field artifact"
observe_both 0 "$T/result-71.rfn" "second valid source/body/result artifact"
observe_both 0 "$T/all-ops.rfn" "all-operation state/branch/edge artifact"
observe_both 251 "$T/mixed-less.rfn" "mixed typed Less carriers"
observe_both 0 "$T/literal-range.rfn" "typed/literal in-range control"
observe_both 251 "$T/literal-overflow.rfn" "typed/literal carrier overflow"

# Source/CKIR and CKIR/result cross-pairs must meet at the exact body meaning.
pack "$T/result-71.comp" "$T/result-71.witness" "$T/canonical.ckir" \
  "$T/canonical.elf" 70 "$T/source71-ckir70.rfn"
pack "$CANONICAL_COMP" "$T/canonical.witness" "$T/result-71.ckir" \
  "$T/result-71.elf" 71 "$T/source70-ckir71.rfn"
observe_both 251 "$T/source71-ckir70.rfn" "source-71/CKIR-70 cross-pair"
observe_both 251 "$T/source70-ckir71.rfn" "source-70/CKIR-71 cross-pair"

# A body-span witness drift and a cross-source field-name drift independently
# retain valid framing but alter the exact identity consumed by body parsing.
python3 - "$T/canonical.witness" "$T/body-drift.witness" \
  "$T/field-drift.witness" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
header = struct.unpack_from("<8sHHHH14I", source)
counts = header[6:17]
strides = (36, 48, 28, 28, 24, 24, 24, 40, 24, 40, 24)
bases = []
cursor = 72
for count, stride in zip(counts, strides):
    bases.append(cursor)
    cursor += count * stride

body = bytearray(source)
block = bases[9]
body_start = struct.unpack_from("<I", body, block + 16)[0]
struct.pack_into("<I", body, block + 16, body_start + 1)
Path(sys.argv[2]).write_bytes(body)

field = bytearray(source)
declaration_name = struct.unpack_from("<I", field, bases[3] + 16)[0]
struct.pack_into("<II", field, bases[6] + 16, declaration_name, 4)
Path(sys.argv[3]).write_bytes(field)
PY
for CASE in body-drift field-drift; do
  pack "$CANONICAL_COMP" "$T/$CASE.witness" "$T/canonical.ckir" \
    "$T/canonical.elf" 70 "$T/$CASE.rfn"
  observe_both 251 "$T/$CASE.rfn" "$CASE witness/body join"
done

# The source-only evaluator's reachable path is intentionally independent of
# both artifact components.  Opaque nonempty replacements remain acceptable
# to it while the lowering join rejects the malformed CKIR header.
printf opaque-ckir > "$T/opaque.ckir"
printf opaque-elf > "$T/opaque.elf"
pack "$CANONICAL_COMP" "$T/canonical.witness" "$T/opaque.ckir" \
  "$T/opaque.elf" 70 "$T/opaque.rfn"
observe "$T/result" 0 "$T/opaque.rfn" "source-only opaque CKIR/ELF independence"
observe "$T/lowering" 251 "$T/opaque.rfn" "resolved lowering malformed CKIR"

pack "$CANONICAL_COMP" "$T/canonical.witness" "$T/canonical.ckir" \
  "$T/canonical.elf" 71 "$T/wrong-result.rfn"
observe "$T/result" 251 "$T/wrong-result.rfn" "wrong source-only full result"

# The claimed result is a full u32, not merely the low-byte process status.
# 326 has the same exit projection as 70 and must still fail the source result
# conjunct; the body/CKIR conjunct deliberately does not own this claim.
pack "$T/all-ops.comp" "$T/all-ops.witness" "$T/all-ops.ckir" \
  "$T/all-ops.elf" 326 "$T/same-exit-result.rfn"
observe "$T/result" 251 "$T/same-exit-result.rfn" "same-exit wrong full result"
observe "$T/lowering" 0 "$T/same-exit-result.rfn" "body/CKIR result independence"

# Direct valid-length row mutations pin every operation/terminator field newly
# owned by this body relation.  The source-only evaluator must remain oblivious
# to each artifact mutation.
python3 - "$T/all-ops.ckir" "$T/row-mutations" <<'PY'
from pathlib import Path
import struct
import sys

raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
out.mkdir()
counts = struct.unpack_from("<12I", raw, 24)
strides = (24, 20, 16, 36, 20, 32, 20, 40, 4, 44)
bases = []
cursor = 72
for count, stride in zip(counts, strides):
    bases.append(cursor)
    cursor += count * stride
if counts[7] == 0 or counts[9] == 0:
    raise SystemExit("all-operation fixture lacks operation or terminator rows")
opcodes = {raw[bases[7] + row * 40 + 12] for row in range(counts[7])}
term_kinds = {raw[bases[9] + row * 44 + 12] for row in range(counts[9])}
if opcodes != set(range(1, 10)):
    raise SystemExit(f"all-operation opcode coverage drifted: {sorted(opcodes)}")
if term_kinds != {2, 4}:
    raise SystemExit(f"all-operation terminator coverage drifted: {sorted(term_kinds)}")

def word(name: str, at: int, value: int) -> None:
    changed = bytearray(raw)
    struct.pack_into("<I", changed, at, value)
    (out / f"{name}.ckir").write_bytes(changed)

def byte(name: str, at: int) -> None:
    changed = bytearray(raw)
    changed[at] ^= 1
    (out / f"{name}.ckir").write_bytes(changed)

op = bases[7]
term = bases[9]
branch = next(
    bases[9] + row * 44
    for row in range(counts[9])
    if raw[bases[9] + row * 44 + 12] == 2
)
word("operation-id", op, 1)
byte("operation-reserved-0", op + 14)
byte("operation-reserved-1", op + 15)
word("operation-immediate-1", op + 36, 1)
word("terminator-id", term, 1)
word("terminator-owner-machine", term + 4, 1)
word("terminator-owner-block", term + 8, 1)
byte("terminator-flags", term + 13)
byte("terminator-reserved-0", term + 14)
byte("terminator-reserved-1", term + 15)
target = struct.unpack_from("<I", raw, branch + 20)[0]
word("terminator-branch-target", branch + 20, target ^ 1)
PY
for NAME in operation-id operation-reserved-0 operation-reserved-1 \
  operation-immediate-1 terminator-id terminator-owner-machine \
  terminator-owner-block terminator-flags terminator-reserved-0 \
  terminator-reserved-1 terminator-branch-target; do
  pack "$T/all-ops.comp" "$T/all-ops.witness" \
    "$T/row-mutations/$NAME.ckir" "$T/all-ops.elf" 70 "$T/$NAME.rfn"
  observe "$T/lowering" 251 "$T/$NAME.rfn" "$NAME body/CKIR relation"
  observe "$T/result" 0 "$T/$NAME.rfn" "$NAME source-result independence"
done

# Representative malformed and resource teeth are phase-local.  CKIR
# exhaustion is irrelevant to the source-only evaluator and therefore remains
# accepted there when the source/result claim is exact.
python3 - "$T/canonical.witness" "$T/bad-witness" "$T/over-witness" \
  "$T/canonical.ckir" "$T/over-ckir" <<'PY'
from pathlib import Path
import struct
import sys

witness = bytearray(Path(sys.argv[1]).read_bytes())
witness[0] ^= 1
Path(sys.argv[2]).write_bytes(witness)
witness = bytearray(Path(sys.argv[1]).read_bytes())
struct.pack_into("<I", witness, 36, 2049)
Path(sys.argv[3]).write_bytes(witness)
ckir = bytearray(Path(sys.argv[4]).read_bytes())
struct.pack_into("<I", ckir, 52, 32769)
Path(sys.argv[5]).write_bytes(ckir)
PY
pack "$CANONICAL_COMP" "$T/bad-witness" "$T/canonical.ckir" \
  "$T/canonical.elf" 70 "$T/bad-witness.rfn"
pack "$CANONICAL_COMP" "$T/over-witness" "$T/canonical.ckir" \
  "$T/canonical.elf" 70 "$T/over-witness.rfn"
pack "$CANONICAL_COMP" "$T/canonical.witness" "$T/over-ckir" \
  "$T/canonical.elf" 70 "$T/over-ckir.rfn"
observe_both 251 "$T/bad-witness.rfn" "malformed witness header"
observe_both 252 "$T/over-witness.rfn" "witness type-count exhaustion"
observe "$T/lowering" 252 "$T/over-ckir.rfn" "CKIR operation-count exhaustion"
observe "$T/result" 0 "$T/over-ckir.rfn" "source-only CKIR exhaustion independence"

ELAPSED=$(($(date +%s)-STARTED))
LOWERING_PROCS=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/lowering.beta")
RESULT_PROCS=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/result.beta")
echo "OMGRFN2 layer 4: resolved bodies -> CKIR and CKIR/ELF-free source result, cross-pairs, and 0/251/252 teeth passed below Delta (${LOWERING_PROCS}/${RESULT_PROCS} procedures, ${ELAPSED}s)"
