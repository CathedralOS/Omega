#!/usr/bin/env sh
# Focused persisted-Beta shared OMGRFN5/OMGRFN6 responsibility-2 gate.
set -eu

START_NS=$(python3 -c 'import time; print(time.time_ns())' 2>/dev/null || echo 0)
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
  *) echo "OMGRFN5 responsibility 2: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN5 responsibility 2: skipped ($TOOL absent)"
    exit 0
  }
done

CORE="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4-source-witness-independent.beta"
ADAPTER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn5-source-witness-independent.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn5_bundle.py"
PACKER6="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn6_bundle.py"
PACKER4="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4_bundle.py"
BUILDER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
SOURCE="$OMEGA_REPO_ROOT/compiler/psi/source/source.omg"
HARNESS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir4-runtime-records/source-unit-harness.omg"
API_HARNESS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir4-runtime-records/source-unit-api-harness.omg"
for REQUIRED in "$CORE" "$ADAPTER" "$PACKER" "$PACKER6" "$PACKER4" "$BUILDER" \
  "$RESOLVER" "$SOURCE" "$HARNESS" "$API_HARNESS" "$OMEGA_PATH_BETA/bc.beta"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN5 responsibility 2: missing $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN5_R2_TEMP:-0}" = 1 ]; then
  echo "OMGRFN5/6 responsibility 2: retained $T" >&2
else
  trap 'rm -rf "$T"' EXIT
fi
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"

awk '1' "$CORE" "$ADAPTER" > "$T/check-core.beta"
awk 'BEGIN { print "proc main() { return omgrfn5_r2_check() }" }' > "$T/main.beta"
awk '1' "$T/check-core.beta" "$T/main.beta" > "$T/check.beta"
PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/check.beta")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN5 responsibility 2: exceeds 128 procedures ($PROCEDURES)" >&2
  exit 1
}
MAX_LOCALS=$(python3 - "$T/check.beta" <<'PY'
import re
import sys
source = open(sys.argv[1], encoding="utf-8").read()
maximum = 0
for match in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{", source, re.M):
    end = source.find("\nproc ", match.end())
    body = source[match.end():end if end >= 0 else len(source)]
    params = sum(bool(item.strip()) for item in match.group(1).split(","))
    maximum = max(maximum, params + len(re.findall(r"\blet\s+[A-Za-z_]\w*", body)))
print(maximum)
PY
)
[ "$MAX_LOCALS" -le 32 ] || {
  echo "OMGRFN5 responsibility 2: exceeds 32 local slots ($MAX_LOCALS)" >&2
  exit 1
}
grep -q 'count>=18000' "$CORE" || {
  echo "OMGRFN5 responsibility 2: token evidence ceiling drifted" >&2
  exit 1
}

# Persisted-Beta evidence: compile the checker through both the stamped native
# compiler and a compiler rebuilt from its own Beta source, then retain exact
# assembly/tape equality before executing both artifacts.
stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"
"$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
BC1_TAPE=$(wc -c < "$T/bc1.tape" | tr -d ' ')
[ $((BC1_TAPE + 4)) -le "$HOLE_SIZE" ] || {
  echo "OMGRFN5 responsibility 2: self-built Beta compiler exceeds seed hole" >&2
  exit 1
}
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
"$T/bc0" < "$T/check.beta" > "$T/native.asm"
"$T/bc1" < "$T/check.beta" > "$T/self.asm"
cmp "$T/native.asm" "$T/self.asm" >/dev/null
"$ASM" < "$T/native.asm" > "$T/native.tape"
"$ASM" < "$T/self.asm" > "$T/self.tape"
cmp "$T/native.tape" "$T/self.tape" >/dev/null
TAPE_BYTES=$(wc -c < "$T/native.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || {
  echo "OMGRFN5 responsibility 2: checker tape exceeds 262140 bytes ($TAPE_BYTES)" >&2
  exit 1
}
stamp_seed "$T/native.tape" "$SEED" "$T/native" >/dev/null 2>&1
stamp_seed "$T/self.tape" "$SEED" "$T/self" >/dev/null 2>&1

# Frozen V4 checker used only for the reverse version-separation tooth.
awk 'BEGIN { print "proc main() { return omgrfn4_r2_check() }" }' > "$T/v4-main.beta"
awk '1' "$CORE" "$T/v4-main.beta" > "$T/v4.beta"
"$T/bc0" < "$T/v4.beta" > "$T/v4.asm"
"$ASM" < "$T/v4.asm" > "$T/v4.tape"
stamp_seed "$T/v4.tape" "$SEED" "$T/v4" >/dev/null 2>&1
BUILD_NS=$(python3 -c 'import time; print(time.time_ns())')

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null

build_pair() { # name owner machine source0 source1
  PAIR_NAME=$1 PAIR_OWNER=$2 PAIR_MACHINE=$3 PAIR_SOURCE0=$4 PAIR_SOURCE1=$5
  python3 -B "$BUILDER" build "$T/$PAIR_NAME.omgc" "$PAIR_OWNER" \
    "$PAIR_MACHINE" "$PAIR_SOURCE0" "$PAIR_SOURCE1"
  "$T/resolver" < "$T/$PAIR_NAME.omgc" > "$T/$PAIR_NAME.witness"
}

build_pair exact SourceUnit bootstrap_runtime_record_probe "$SOURCE" "$HARNESS"
build_pair api SourceUnit bootstrap_source_api_probe "$SOURCE" "$API_HARNESS"

# Compact same-module OMGRSW2 carrier. The shared resolver emits its canonical
# role-3 rows directly. The fallback below can complete the same untrusted
# fixture when this focused gate is run beside a frozen OMGRSW1-only resolver;
# either way, the persisted checker independently reconstructs every byte.
python3 -B - "$T/field-source.omg" "$T/field-harness.omg" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_text("""\
data Child {}
data Parent {
    child: Child;
    flag: bool;
    full: u32;
    trap: u32 in Trapping;
    byte: u8;
    bound: u32 [0..=65536];
    bytes: [u8; 65536] in Trapping;
}

machine Child::store(&mut self, value: u8) {}
machine Child::load(&self) -> u8 { 70 }
machine Parent::noop(&mut self) {}
""", encoding="ascii")
Path(sys.argv[2]).write_text("""\
machine Parent::run(&mut self) -> u8 {
    self.noop();
    self.child.store(70);
    self.child.load()
}
""", encoding="ascii")
PY
build_pair field Parent run "$T/field-source.omg" "$T/field-harness.omg"
python3 -B - "$T/field.witness" "$T/field-source.omg" "$T/field-harness.omg" <<'PY'
from pathlib import Path
import struct, sys
p = Path(sys.argv[1]); raw = bytearray(p.read_bytes())
sources = (Path(sys.argv[2]).read_bytes(), Path(sys.argv[3]).read_bytes())
head = struct.unpack_from("<8sHHHH14I", raw)
counts = list(head[6:17])
strides = (36, 48, 28, 28, 24, 24, 24, 40, 24, 40, 24)
offsets = []
at = 72
for count, stride in zip(counts, strides):
    offsets.append(at); at += count * stride
if at != len(raw): raise SystemExit("OMGRFN6 R2 fixture witness extent")
if head[0] == b"OMGRSW2\0":
    role3 = [i for i in range(counts[2]) if raw[offsets[2] + i * 28 + 8] == 3]
    if len(role3) != 3: raise SystemExit("OMGRFN6 R2 resolver role-3 census")
    raise SystemExit(0)
if head[0] != b"OMGRSW1\0" or head[1:5] != (1, 0, 0, 72):
    raise SystemExit("OMGRFN6 R2 fixture base identity")
bindings = []
for i in range(counts[2]):
    row = bytearray(raw[offsets[2] + i * 28:offsets[2] + (i + 1) * 28])
    bindings.append(row)
declarations = {}
for i in range(counts[3]):
    row = struct.unpack_from("<IBBH5I", raw, offsets[3] + i * 28)
    name = sources[row[4]][row[6]:row[6] + row[7]]
    if row[1] == 2: declarations[name] = i
for name in (b"store", b"load"):
    needle = b"self.child." + name + b"("
    start = sources[1].index(needle) + len(b"self.child.")
    bindings.append(bytearray(struct.pack(
        "<IIBBH4I", 0, 1, 3, 2, 0, start, len(name), declarations[name], 0xffffffff
    )))
bindings.sort(key=lambda row: (
    struct.unpack_from("<I", row, 4)[0],
    struct.unpack_from("<I", row, 12)[0], row[8]
))
for i, row in enumerate(bindings): struct.pack_into("<I", row, 0, i)
tail = raw[offsets[2] + counts[2] * 28:]
raw = raw[:offsets[2]] + b"".join(bindings) + tail
raw[:8] = b"OMGRSW2\0"
struct.pack_into("<H", raw, 8, 2)
struct.pack_into("<I", raw, 16, len(raw))
struct.pack_into("<I", raw, 28, len(bindings))
p.write_bytes(raw)
PY

# Resolve parameter spans back through their owning machine/block and source.
# This locks in the corrected resolver behavior rather than merely comparing
# two byte strings produced by the same implementation.
python3 - "$T/exact.witness" "$SOURCE" "$HARNESS" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
sources = (Path(sys.argv[2]).read_bytes(), Path(sys.argv[3]).read_bytes())
head = struct.unpack_from("<8sHHHH14I", raw)
counts = head[6:17]
strides = (36, 48, 28, 28, 24, 24, 24, 40, 24, 40, 24)
offsets = []
at = 72
for count, stride in zip(counts, strides):
    offsets.append(at); at += count * stride
if at != len(raw) or counts != (2, 0, 9, 8, 10, 4, 9, 4, 3, 9, 1):
    raise SystemExit("OMGRFN5 R2 exact witness census")
decls, machines, mparams, blocks, bparams = (offsets[i] for i in (3, 7, 8, 9, 10))
def machine_source(mid):
    declaration = struct.unpack_from("<I", raw, machines + mid * 40 + 4)[0]
    return struct.unpack_from("<I", raw, decls + declaration * 28 + 8)[0]
observed = []
for i in range(counts[8]):
    row = struct.unpack_from("<6I", raw, mparams + i * 24)
    source = sources[machine_source(row[1])]
    observed.append(source[row[4]:row[4] + row[5]])
block_row = struct.unpack_from("<10I", raw, blocks + 8 * 40)
row = struct.unpack_from("<6I", raw, bparams)
source = sources[machine_source(block_row[1])]
observed.append(source[row[4]:row[4] + row[5]])
if observed != [b"id", b"byte", b"index", b"runtime_scalar"]:
    raise SystemExit(f"OMGRFN5 R2 parameter spans: {observed!r}")
PY

# The second carrier changes the result type, call-binding census, and absence
# of block parameters without changing the logical module.  Inspect those
# facts from table relationships rather than relying on fixed IDs.
python3 - "$T/api.witness" "$SOURCE" "$API_HARNESS" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
sources = (Path(sys.argv[2]).read_bytes(), Path(sys.argv[3]).read_bytes())
head = struct.unpack_from("<8sHHHH14I", raw)
counts = head[6:17]
strides = (36, 48, 28, 28, 24, 24, 24, 40, 24, 40, 24)
offsets = []
at = 72
for count, stride in zip(counts, strides):
    offsets.append(at); at += count * stride
if at != len(raw) or counts[10] != 0:
    raise SystemExit("OMGRFN5 R2 SourceUnit API extent/block-parameter census")
bindings, declarations, types, machines = (offsets[i] for i in (2, 3, 4, 7))
def declaration(did):
    if did >= counts[3]: raise SystemExit("OMGRFN5 R2 declaration range")
    return struct.unpack_from("<IBBH5I", raw, declarations + did * 28)
def declaration_name(did):
    row = declaration(did)
    source, start, length = row[4], row[6], row[7]
    return sources[source][start:start + length]
role3 = []
for i in range(counts[2]):
    row = struct.unpack_from("<IIBBH4I", raw, bindings + i * 28)
    if row[2] == 3:
        reference = sources[row[1]][row[5]:row[5] + row[6]]
        target = declaration_name(row[7])
        role3.append((reference, target))
expected = [(b"clear", b"clear"), (b"append", b"append"),
            (b"byte_or_nul", b"byte_or_nul")]
if role3 != expected:
    raise SystemExit(f"OMGRFN5 R2 SourceUnit API role-3 rows: {role3!r}")
root = head[17]
if root >= counts[7]: raise SystemExit("OMGRFN5 R2 SourceUnit API root range")
machine = struct.unpack_from("<3IBBH6I", raw, machines + root * 40)
if declaration_name(machine[1]) != b"bootstrap_source_api_probe":
    raise SystemExit("OMGRFN5 R2 SourceUnit API selected root")
result = machine[6]
if result >= counts[4]: raise SystemExit("OMGRFN5 R2 SourceUnit API result range")
type_at = types + result * 24
if raw[type_at + 4] != 1 or struct.unpack_from("<I", raw, type_at + 20)[0] != 255:
    raise SystemExit("OMGRFN5 R2 SourceUnit API root result is not derived u8")
PY

printf 'opaque-CKIR4' > "$T/ckir"
printf 'opaque-ELF' > "$T/elf"
: > "$T/empty"

pack() { # output compilation witness
  python3 "$PACKER" "$2" "$3" "$T/ckir" "$T/elf" --result 70 > "$T/$1.rfn"
}
pack6() { # output compilation witness
  python3 "$PACKER6" "$2" "$3" "$T/ckir" "$T/elf" --result 70 > "$T/$1.rfn"
}
pack exact "$T/exact.omgc" "$T/exact.witness"
pack api "$T/api.omgc" "$T/api.witness"
pack6 field "$T/field.omgc" "$T/field.witness"
python3 - "$T/api.witness" "$T/api-forged-block-param.witness" <<'PY'
from pathlib import Path
import struct
import sys
raw = bytearray(Path(sys.argv[1]).read_bytes())
declared = struct.unpack_from("<I", raw, 16)[0]
struct.pack_into("<I", raw, 16, declared + 24)
struct.pack_into("<I", raw, 60, 1)
raw.extend(struct.pack("<6I", 0, 0, 0, 0, 0, 0))
Path(sys.argv[2]).write_bytes(raw)
PY
pack api-forged-block-param "$T/api.omgc" "$T/api-forged-block-param.witness"

observe_one() { # exe expected input label
  EXE=$1 EXPECTED=$2 INPUT=$3 LABEL=$4
  set +e
  "$EXE" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN5 responsibility 2: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN5 responsibility 2: $LABEL published stdout" >&2
    exit 1
  }
}
observe() {
  observe_one "$T/native" "$1" "$2" "$3 (native)"
  observe_one "$T/self" "$1" "$2" "$3 (self)"
}
observe 0 "$T/exact.rfn" "exact source+harness resolution"
observe 0 "$T/api.rfn" "complete SourceUnit API resolution"
observe 0 "$T/field.rfn" "OMGRFN6 direct field-receiver resolution"
observe 251 "$T/api-forged-block-param.rfn" \
  "forged SourceUnit API block-parameter row/count"

# Fully source-derived identifiers, including every corrected parameter-name
# span, and an independently authored field reordering.
python3 - "$SOURCE" "$HARNESS" "$T/renamed-source.omg" "$T/renamed-harness.omg" "$T/reordered-source.omg" <<'PY'
from pathlib import Path
import sys
source = Path(sys.argv[1]).read_text(encoding="utf-8")
harness = Path(sys.argv[2]).read_text(encoding="utf-8")
for old, new in (
    ("SourceSpan", "InputRange"), ("SourceId", "InputId"),
    ("Span", "ByteRange"), ("SourceUnit", "InputUnit"),
    ("byte_or_nul", "read_or_zero"), ("append", "push_byte"),
    ("clear", "reset"),
):
    source = source.replace(old, new)
    harness = harness.replace(old, new)
source = source.replace(
    "machine InputUnit::reset(&mut self, id: InputId)",
    "machine InputUnit::reset(&mut self, identity: InputId)",
).replace("self.id = id;", "self.id = identity;")
source = source.replace("byte: u8", "octet: u8", 1).replace("= byte;", "= octet;")
source = source.replace("index: u32 in Trapping", "coordinate: u32 in Trapping", 1).replace("[index]", "[coordinate]")
harness = harness.replace("runtime_scalar", "runtime_value")
harness = harness.replace("bootstrap_runtime_record_probe", "bootstrap_runtime_value_probe")
Path(sys.argv[3]).write_text(source, encoding="utf-8")
Path(sys.argv[4]).write_text(harness, encoding="utf-8")
old = """    length: u32 [0..=65536];
    last_retained: bool;
"""
new = """    last_retained: bool;
    length: u32 [0..=65536];
"""
original = Path(sys.argv[1]).read_text(encoding="utf-8")
if original.count(old) != 1:
    raise SystemExit("OMGRFN5 R2 reorder anchor")
Path(sys.argv[5]).write_text(original.replace(old, new), encoding="utf-8")
PY
build_pair renamed InputUnit bootstrap_runtime_value_probe "$T/renamed-source.omg" "$T/renamed-harness.omg"
build_pair reordered SourceUnit bootstrap_runtime_record_probe "$T/reordered-source.omg" "$HARNESS"
python3 - "$T/renamed.witness" "$T/renamed-source.omg" "$T/renamed-harness.omg" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
sources = (Path(sys.argv[2]).read_bytes(), Path(sys.argv[3]).read_bytes())
head = struct.unpack_from("<8sHHHH14I", raw)
counts = head[6:17]
strides = (36, 48, 28, 28, 24, 24, 24, 40, 24, 40, 24)
offsets = []
at = 72
for count, stride in zip(counts, strides): offsets.append(at); at += count * stride
def machine_source(mid):
    declaration = struct.unpack_from("<I", raw, offsets[7] + mid * 40 + 4)[0]
    return struct.unpack_from("<I", raw, offsets[3] + declaration * 28 + 8)[0]
names = []
for i in range(counts[8]):
    row = struct.unpack_from("<6I", raw, offsets[8] + i * 24)
    source = sources[machine_source(row[1])]
    names.append(source[row[4]:row[4] + row[5]])
block = struct.unpack_from("<10I", raw, offsets[9] + 8 * 40)
row = struct.unpack_from("<6I", raw, offsets[10])
source = sources[machine_source(block[1])]
names.append(source[row[4]:row[4] + row[5]])
if names != [b"identity", b"octet", b"coordinate", b"runtime_value"]:
    raise SystemExit(f"OMGRFN5 R2 renamed parameter spans: {names!r}")
PY
pack renamed "$T/renamed.omgc" "$T/renamed.witness"
pack reordered "$T/reordered.omgc" "$T/reordered.witness"
observe 0 "$T/renamed.rfn" "fully renamed declarations/references/parameters"
observe 0 "$T/reordered.rfn" "authored field reordering"

pack cross-renamed "$T/exact.omgc" "$T/renamed.witness"
pack cross-reordered "$T/exact.omgc" "$T/reordered.witness"
pack cross-api "$T/exact.omgc" "$T/api.witness"
pack api-cross "$T/api.omgc" "$T/exact.witness"
observe 251 "$T/cross-renamed.rfn" "renamed valid cross-pair"
observe 251 "$T/cross-reordered.rfn" "reordered valid cross-pair"
observe 251 "$T/cross-api.rfn" "exact source with SourceUnit API witness"
observe 251 "$T/api-cross.rfn" "SourceUnit API source with exact witness"

# Schema identities are strict even though one executable shares their code.
# OMGRSW2 additionally requires at least one independently derived direct
# field-receiver call rather than serving as a cosmetic header rewrite.
python3 -B - "$T/api.witness" "$T/api-v2.witness" <<'PY'
from pathlib import Path
import struct, sys
raw = bytearray(Path(sys.argv[1]).read_bytes())
raw[:8] = b"OMGRSW2\0"; struct.pack_into("<H", raw, 8, 2)
Path(sys.argv[2]).write_bytes(raw)
PY
pack field-as-v5 "$T/field.omgc" "$T/field.witness"
python3 -B - "$T/exact.omgc" "$T/exact.witness" "$T/ckir" "$T/elf" "$T/exact-as-v6.rfn" <<'PY'
from pathlib import Path
import struct, sys
omg, witness, ckir, elf = (Path(name).read_bytes() for name in sys.argv[1:5])
header = struct.pack("<8s8I", b"OMGRFN6\0", 6, 1, len(omg), len(witness),
                     len(ckir), len(elf), 70, 70)
Path(sys.argv[5]).write_bytes(header + omg + witness + ckir + elf)
PY
pack6 api-v2-no-field "$T/api.omgc" "$T/api-v2.witness"
observe 251 "$T/field-as-v5.rfn" "OMGRFN5 rejects OMGRSW2"
observe 251 "$T/exact-as-v6.rfn" "OMGRFN6 rejects OMGRSW1"
observe 251 "$T/api-v2-no-field.rfn" "OMGRSW2 requires a field call"

# Unknown and chained/computed receiver spellings fail from source semantics,
# before a mismatched but otherwise valid OMGRSW2 can supply authority.
python3 -B - "$T/field-harness.omg" "$T/field-unknown.omg" "$T/field-chained.omg" <<'PY'
from pathlib import Path
import sys
s = Path(sys.argv[1]).read_text(encoding="ascii")
Path(sys.argv[2]).write_text(s.replace("self.child.load()", "self.absent.load()"), encoding="ascii")
Path(sys.argv[3]).write_text(s.replace("self.child.load()", "self.child.value.load()"), encoding="ascii")
PY
python3 -B "$BUILDER" build "$T/field-unknown.omgc" Parent run "$T/field-source.omg" "$T/field-unknown.omg"
python3 -B "$BUILDER" build "$T/field-chained.omgc" Parent run "$T/field-source.omg" "$T/field-chained.omg"
pack6 field-unknown "$T/field-unknown.omgc" "$T/field.witness"
pack6 field-chained "$T/field-chained.omgc" "$T/field.witness"
observe 251 "$T/field-unknown.rfn" "unknown direct receiver field"
observe 251 "$T/field-chained.rfn" "chained field receiver"

make_mutations() { # carrier name
python3 - "$T/$1.witness" "$T/$1-mutations" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2]); out.mkdir()
names = []
head = struct.unpack_from("<8sHHHH14I", raw)
counts = head[6:17]
strides = (36, 48, 28, 28, 24, 24, 24, 40, 24, 40, 24)
offsets = []
at = 72
for count, stride in zip(counts, strides): offsets.append(at); at += count * stride
if at != len(raw): raise SystemExit("OMGRFN5 R2 mutation input extent")
def put(name, data):
    out.joinpath(name + ".witness").write_bytes(data)
    names.append(name)
def bump(name, offset):
    data = bytearray(raw)
    struct.pack_into("<I", data, offset, struct.unpack_from("<I", data, offset)[0] + 1)
    put(name, data)
for index in range(counts[8]):
    bump(f"machine-param-{index}-start", offsets[8] + index * 24 + 16)
    bump(f"machine-param-{index}-length", offsets[8] + index * 24 + 20)
for index in range(counts[10]):
    bump(f"block-param-{index}-start", offsets[10] + index * 24 + 16)
    bump(f"block-param-{index}-length", offsets[10] + index * 24 + 20)
role3 = [i for i in range(counts[2]) if raw[offsets[2] + i * 28 + 8] == 3]
for index in role3:
    r = offsets[2] + index * 28
    bump(f"role3-{index}-span", r + 12)
    data = bytearray(raw)
    target = struct.unpack_from("<I", data, r + 20)[0]
    struct.pack_into("<I", data, r + 20, target ^ 1)
    put(f"role3-{index}-target", data)
    data = bytearray(raw); data[r + 8] = 2; put(f"role3-{index}-role", data)
if counts[2] < 2: raise SystemExit("OMGRFN5 R2 binding-order mutation needs two rows")
left = offsets[2]
r = offsets[2] + 28
data = bytearray(raw)
a, b = bytes(data[left + 4:left + 28]), bytes(data[r + 4:r + 28])
data[left + 4:left + 28], data[r + 4:r + 28] = b, a
put("binding-order", data)
root = head[17]
if root >= counts[7]: raise SystemExit("OMGRFN5 R2 selected root range")
result = offsets[7] + root * 40 + 16
if struct.unpack_from("<I", raw, result)[0] != 0xffffffff:
    bump("root-result", result)
out.joinpath("manifest").write_text("\n".join(names) + "\n", encoding="ascii")
PY
}
make_mutations exact
make_mutations api
make_mutations field
for CARRIER in exact api; do
  while IFS= read -r NAME; do
    pack "$CARRIER-mutation-$NAME" "$T/$CARRIER.omgc" \
      "$T/$CARRIER-mutations/$NAME.witness"
    observe 251 "$T/$CARRIER-mutation-$NAME.rfn" \
      "$CARRIER $NAME mutation"
  done < "$T/$CARRIER-mutations/manifest"
done
while IFS= read -r NAME; do
  pack6 "field-mutation-$NAME" "$T/field.omgc" \
    "$T/field-mutations/$NAME.witness"
  observe 251 "$T/field-mutation-$NAME.rfn" "OMGRFN6 field $NAME mutation"
done < "$T/field-mutations/manifest"

# Missing role-3 resolution is independently rejected from source, and the
# production resolver agrees without publishing a witness.
python3 - "$HARNESS" "$T/missing-harness.omg" <<'PY'
from pathlib import Path
import sys
raw = Path(sys.argv[1]).read_bytes()
if raw.count(b"self.clear(") != 1:
    raise SystemExit("OMGRFN5 R2 missing-call anchor")
Path(sys.argv[2]).write_bytes(raw.replace(b"self.clear(", b"self.absent(", 1))
PY
python3 -B "$BUILDER" build "$T/missing.omgc" SourceUnit \
  bootstrap_runtime_record_probe "$SOURCE" "$T/missing-harness.omg"
pack missing "$T/missing.omgc" "$T/exact.witness"
observe 251 "$T/missing.rfn" "missing source call binding"
observe_one "$T/resolver" 251 "$T/missing.omgc" "resolver missing source call binding"

# CKIR4, ELF, and claimed result are physically present but propositionally
# opaque to responsibility 2.
printf 'changed-CKIR4-with-no-resolution-authority' > "$T/changed.ckir"
printf 'changed-ELF-with-no-resolution-authority' > "$T/changed.elf"
python3 "$PACKER" "$T/exact.omgc" "$T/exact.witness" \
  "$T/changed.ckir" "$T/changed.elf" --result 71 > "$T/opaque.rfn"
observe 0 "$T/opaque.rfn" "CKIR4/ELF/result opacity"
python3 "$PACKER" "$T/exact.omgc" "$T/exact.witness" \
  "$T/changed.ckir" "$T/empty" --library > "$T/library.rfn"
observe 0 "$T/library.rfn" "library result framing"

# V4/V5 separation in both directions and local resource precedence.
python3 "$PACKER4" "$T/exact.omgc" "$T/exact.witness" \
  "$T/ckir" "$T/elf" --result 70 > "$T/v4.rfn"
observe 251 "$T/v4.rfn" "OMGRFN4 cross-version carrier"
observe_one "$T/v4" 251 "$T/exact.rfn" "frozen OMGRFN4 checker rejects OMGRFN5"
python3 - "$T/exact.rfn" "$T/bad-version.rfn" "$T/witness-over.rfn" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
bad = bytearray(raw); struct.pack_into("<I", bad, 8, 4); Path(sys.argv[2]).write_bytes(bad)
over = bytearray(raw); struct.pack_into("<I", over, 20, 524289); Path(sys.argv[3]).write_bytes(over)
PY
observe 251 "$T/bad-version.rfn" "malformed V5 version"
observe 252 "$T/witness-over.rfn" "declared witness exhaustion"
python3 -B - "$T/field.rfn" "$T/field-bad-version.rfn" "$T/field-witness-over.rfn" <<'PY'
from pathlib import Path
import struct, sys
raw = Path(sys.argv[1]).read_bytes()
bad = bytearray(raw); struct.pack_into("<I", bad, 8, 5); Path(sys.argv[2]).write_bytes(bad)
over = bytearray(raw); struct.pack_into("<I", over, 20, 524289); Path(sys.argv[3]).write_bytes(over)
PY
observe 251 "$T/field-bad-version.rfn" "malformed V6 version"
observe 252 "$T/field-witness-over.rfn" "V6 declared witness exhaustion"

python3 - "$T/token-over.omg" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_text(";" * 18001, encoding="ascii")
PY
python3 -B "$BUILDER" build "$T/token-over.omgc" SourceUnit \
  bootstrap_runtime_record_probe "$T/token-over.omg" "$HARNESS"
pack token-over "$T/token-over.omgc" "$T/exact.witness"
observe 252 "$T/token-over.rfn" "18001-token source evidence exhaustion"

END_NS=$(python3 -c 'import time; print(time.time_ns())')
BUILD_MS=$(( (BUILD_NS - START_NS) / 1000000 ))
RUN_MS=$(( (END_NS - BUILD_NS) / 1000000 ))
TOTAL_MS=$(( (END_NS - START_NS) / 1000000 ))
echo "OMGRFN5/6 responsibility 2: frozen OMGRSW1 carriers and same-module OMGRSW2 self.field.machine carrier, direct shared/mutable Unit/scalar calls, strict V5/V6 cross-pairs, field/role-3 mutations, unknown/chained rejection, opacity, and 0/251/252 passed native/self"
echo "OMGRFN5/6 responsibility 2 resources: ${PROCEDURES}/128 procedures; ${MAX_LOCALS}/32 locals; ${TAPE_BYTES}/262140 checker tape bytes; ${BC1_TAPE}+4/${HOLE_SIZE} self-Beta bytes; 18000 tokens/source; build=${BUILD_MS}ms matrix=${RUN_MS}ms total=${TOTAL_MS}ms"
