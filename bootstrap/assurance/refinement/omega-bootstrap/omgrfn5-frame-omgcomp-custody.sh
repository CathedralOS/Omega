#!/usr/bin/env sh
# Focused lower-rooted OMGRFN5 responsibility-1 framing and source-custody gate.
set -eu

STARTED=$(date +%s)
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN5 responsibility 1: skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v python3 >/dev/null 2>&1 || {
  echo "OMGRFN5 responsibility 1: skipped (python3 absent)"
  exit 0
}

CORE="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4-frame-omgcomp-custody.beta"
ADAPTER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn5-frame-omgcomp-custody.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn5_bundle.py"
PACKER4="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4_bundle.py"
# Frame/source custody needs only the frozen OMGCOMP packer; it deliberately
# does not depend on an in-progress CKIR4 producer fixture.
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py"
SOURCE="$OMEGA_REPO_ROOT/compiler/psi/source/source.omg"
HARNESS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir4-runtime-records/source-unit-harness.omg"
for REQUIRED in "$CORE" "$ADAPTER" "$PACKER" "$PACKER4" "$FIXTURE" \
  "$SOURCE" "$HARNESS" "$OMEGA_PATH_BETA/bc.beta"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN5 responsibility 1: missing $REQUIRED" >&2
    exit 1
  }
done

PROCEDURES=$((
  $(awk '/^proc / { count += 1 } END { print count + 0 }' "$CORE") +
  $(awk '/^proc / { count += 1 } END { print count + 0 }' "$ADAPTER") + 1
))
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN5 responsibility 1: checker exceeds 128 procedures ($PROCEDURES)" >&2
  exit 1
}
MAX_LOCALS=$(python3 - "$CORE" "$ADAPTER" <<'PY'
import re
import sys

maximum = 0
for path in sys.argv[1:]:
    source = open(path, encoding="utf-8").read()
    for match in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{", source, re.M):
        end = source.find("\nproc ", match.end())
        body = source[match.end():end if end >= 0 else len(source)]
        params = sum(bool(item.strip()) for item in match.group(1).split(","))
        maximum = max(maximum, params + len(re.findall(r"\blet\s+[A-Za-z_]\w*", body)))
print(maximum)
PY
)
[ "$MAX_LOCALS" -le 32 ] || {
  echo "OMGRFN5 responsibility 1: checker exceeds 32 local slots ($MAX_LOCALS)" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$T/bc0" >/dev/null

# Rebuild the Beta compiler once and require the responsibility source to have
# identical native-lattice and self-hosted compilations.
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"
"$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
BC1_TAPE=$(wc -c < "$T/bc1.tape" | tr -d ' ')
[ $((BC1_TAPE + 4)) -le "$HOLE_SIZE" ] || {
  echo "OMGRFN5 responsibility 1: self-built Beta compiler exceeds seed hole" >&2
  exit 1
}
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1

build_source() { # output-main output-source
  MAIN=$1
  OUTPUT=$2
  cat "$CORE" "$ADAPTER" "$MAIN" > "$OUTPUT"
}

printf '\nproc main() { return omgrfn5_layer1_check() }\n' > "$T/main.beta"
build_source "$T/main.beta" "$T/check.beta"
"$T/bc0" < "$T/check.beta" > "$T/native.asm"
"$T/bc1" < "$T/check.beta" > "$T/self.asm"
cmp "$T/native.asm" "$T/self.asm" >/dev/null
"$ASM" < "$T/native.asm" > "$T/native.tape"
"$ASM" < "$T/self.asm" > "$T/self.tape"
cmp "$T/native.tape" "$T/self.tape" >/dev/null
stamp_seed "$T/native.tape" "$SEED" "$T/native" >/dev/null 2>&1
stamp_seed "$T/self.tape" "$SEED" "$T/self" >/dev/null 2>&1

printf '\nproc main() { return omgrfn4_layer1_check() }\n' > "$T/v4-main.beta"
cat "$CORE" "$T/v4-main.beta" > "$T/v4-check.beta"
"$T/bc0" < "$T/v4-check.beta" > "$T/v4-check.asm"
"$ASM" < "$T/v4-check.asm" > "$T/v4-check.tape"
stamp_seed "$T/v4-check.tape" "$SEED" "$T/v4-check" >/dev/null 2>&1

# Exact product source plus its same-logical-module runtime-record harness.
python3 -B "$FIXTURE" build "$T/exact.omgc" SourceUnit \
  bootstrap_runtime_record_probe "$SOURCE" "$HARNESS"
printf 'opaque-OMGRSW1' > "$T/witness"
printf 'opaque-CKIR4' > "$T/ckir"
printf 'opaque-ELF' > "$T/elf"
: > "$T/empty"
python3 "$PACKER" "$T/exact.omgc" "$T/witness" "$T/ckir" "$T/elf" \
  --result 70 > "$T/entry.rfn"
python3 "$PACKER" "$T/exact.omgc" "$T/witness" "$T/ckir" "$T/empty" \
  --library > "$T/library.rfn"
python3 "$PACKER4" "$T/exact.omgc" "$T/witness" "$T/ckir" "$T/elf" \
  --result 70 > "$T/version4.rfn"

observe_one() { # executable expected input label
  EXE=$1
  EXPECTED=$2
  INPUT=$3
  LABEL=$4
  set +e
  "$EXE" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN5 responsibility 1: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN5 responsibility 1: $LABEL published stdout" >&2
    exit 1
  }
}

observe() {
  observe_one "$T/native" "$1" "$2" "$3 (native)"
  observe_one "$T/self" "$1" "$2" "$3 (self)"
}

observe 0 "$T/entry.rfn" "exact source entry frame"
observe 0 "$T/library.rfn" "exact source library frame"
observe 251 "$T/version4.rfn" "frozen OMGRFN4 cross-version frame"
observe_one "$T/v4-check" 251 "$T/entry.rfn" \
  "frozen OMGRFN4 checker rejects OMGRFN5"

# Independently derive every source-ID to nested-bundle content extent and
# require the persisted checker to publish the same custody projection.
python3 - "$T/exact.omgc" "$T/extent-main.beta" <<'PY'
from pathlib import Path
import struct
import sys

data = Path(sys.argv[1]).read_bytes()
fields = struct.unpack_from("<8sHHHH12I", data)
bundle_length, package_count, source_count = fields[6], fields[9], fields[10]
cursor = len(data) - bundle_length + 16
bundle_extents = []
for _ in range(source_count):
    label_length, content_length = struct.unpack_from("<II", data, cursor)
    content_start = cursor + 8 + label_length
    bundle_extents.append((content_start, content_length))
    cursor = content_start + content_length
source_table = 64 + package_count * 48
expected = []
for source_id in range(source_count):
    row = source_table + source_id * 20
    dense, _, bundle_id, _, reserved = struct.unpack_from("<IIIII", data, row)
    assert dense == source_id and reserved == 0
    expected.append(bundle_extents[bundle_id])

lines = ["", "proc main() {", "    let status = omgrfn5_layer1_check()",
         "    to failed when (status != 0)", "    to span_0"]
for source_id, (start, length) in enumerate(expected):
    lines += [
        f"    state span_{source_id} {{",
        f"        to bad when (omgrfn5_source_content_start({source_id}) != {start})",
        f"        to length_{source_id}", "    }",
        f"    state length_{source_id} {{",
        f"        to bad when (omgrfn5_source_content_length({source_id}) != {length})",
        f"        to span_{source_id + 1}" if source_id + 1 < len(expected)
        else "        to success", "    }",
    ]
lines += ["    state failed { return status }", "    state bad { return 251 }",
          "    state success { return 0 }", "}"]
Path(sys.argv[2]).write_text("\n".join(lines) + "\n", encoding="ascii")
PY
build_source "$T/extent-main.beta" "$T/extent-check.beta"
"$T/bc0" < "$T/extent-check.beta" > "$T/extent-check.asm"
"$ASM" < "$T/extent-check.asm" > "$T/extent-check.tape"
stamp_seed "$T/extent-check.tape" "$SEED" "$T/extent-check" >/dev/null 2>&1
observe_one "$T/extent-check" 0 "$T/entry.rfn" "exact source content extents"

python3 - "$T/entry.rfn" "$T/library.rfn" "$T/cases" <<'PY'
from pathlib import Path
import struct
import sys

canonical = Path(sys.argv[1]).read_bytes()
library = Path(sys.argv[2]).read_bytes()
out = Path(sys.argv[3]); out.mkdir()

def put(name, value): (out / name).write_bytes(value)
def changed(at, value):
    result = bytearray(canonical); result[at] = value; return bytes(result)
def u32(at, value):
    result = bytearray(canonical); struct.pack_into("<I", result, at, value); return bytes(result)

put("bad-magic", changed(0, ord("X")))
put("bad-version", u32(8, 4))
put("bad-flags", u32(12, 2))
put("empty-omgcomp", u32(16, 0))
put("empty-witness", u32(20, 0))
put("empty-ckir", u32(24, 0))
put("bad-exit", u32(36, 71))
put("truncated", canonical[:-1])
put("trailing", canonical + b"\0")
put("library-with-elf", u32(12, 0))
entry_without_elf = bytearray(library); struct.pack_into("<I", entry_without_elf, 12, 1)
put("entry-without-elf", bytes(entry_without_elf))
library_result = bytearray(library); struct.pack_into("<I", library_result, 32, 70)
put("library-with-result", bytes(library_result))

put("over-omgcomp", u32(16, 267_281))
put("over-witness", u32(20, 524_289))
put("over-ckir", u32(24, 2_522_193))
put("over-elf", u32(28, 1_183_745))

omgcomp_length, witness_length, ckir_length = struct.unpack_from("<III", canonical, 16)
omgcomp_at = 40
omgcomp = canonical[omgcomp_at:omgcomp_at + omgcomp_length]
header = struct.unpack_from("<8sHHHH12I", omgcomp)
bundle_at = len(omgcomp) - header[6]
source_count = header[10]
bad_comp = bytearray(canonical); bad_comp[omgcomp_at] ^= 1
put("bad-omgcomp-magic", bytes(bad_comp))
bad_bundle = bytearray(canonical); bad_bundle[omgcomp_at + bundle_at] ^= 1
put("bad-bundle-magic", bytes(bad_bundle))
bad_bundle_count = bytearray(canonical)
struct.pack_into("<I", bad_bundle_count, omgcomp_at + bundle_at + 12, source_count + 1)
put("bad-bundle-count", bytes(bad_bundle_count))

for name, at in (
    ("changed-witness", 40 + omgcomp_length),
    ("changed-ckir", 40 + omgcomp_length + witness_length),
    ("changed-elf", 40 + omgcomp_length + witness_length + ckir_length),
): put(name, changed(at, canonical[at] ^ 1))
claims = bytearray(canonical); struct.pack_into("<II", claims, 32, 71, 71)
put("changed-valid-claims", bytes(claims))
cursor = bundle_at + 16
label_length, content_length = struct.unpack_from("<II", omgcomp, cursor)
assert content_length > 0
content_at = omgcomp_at + cursor + 8 + label_length
put("changed-source-content", changed(content_at, canonical[content_at] ^ 1))
PY

for CASE in bad-magic bad-version bad-flags empty-omgcomp empty-witness \
  empty-ckir bad-exit truncated trailing library-with-elf entry-without-elf \
  library-with-result bad-omgcomp-magic bad-bundle-magic bad-bundle-count; do
  observe 251 "$T/cases/$CASE" "$CASE"
done
for CASE in over-omgcomp over-witness over-ckir over-elf; do
  observe 252 "$T/cases/$CASE" "$CASE"
done
for CASE in changed-witness changed-ckir changed-elf changed-valid-claims \
  changed-source-content; do
  observe 0 "$T/cases/$CASE" "$CASE"
done

# Exact accepted opaque-component ceilings exercise every checked offset add.
python3 - "$T/max-witness" "$T/max-ckir" "$T/max-elf" <<'PY'
from pathlib import Path
import sys
for name, size, byte in zip(sys.argv[1:], (524_288, 2_522_192, 1_183_744), b"wce"):
    Path(name).write_bytes(bytes((byte,)) * size)
PY
python3 "$PACKER" "$T/exact.omgc" "$T/max-witness" "$T/max-ckir" \
  "$T/max-elf" --result 70 > "$T/component-max.rfn"
observe 0 "$T/component-max.rfn" "exact witness/CKIR4/ELF component ceilings"

python3 - "$T/exact-ceiling" "$T/adjacent" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_bytes(b"\0" * 4_497_544)
Path(sys.argv[2]).write_bytes(b"\0" * 4_497_545)
PY
observe 251 "$T/exact-ceiling" "exact raw whole-frame ceiling"
observe 252 "$T/adjacent" "first byte beyond whole-frame ceiling"

TAPE_BYTES=$(wc -c < "$T/native.tape" | tr -d ' ')
SOURCE_BYTES=$((
  $(wc -c < "$CORE" | tr -d ' ') +
  $(wc -c < "$ADAPTER" | tr -d ' ') +
  $(wc -c < "$T/main.beta" | tr -d ' ')
))
OMGCOMP_BYTES=$(wc -c < "$T/exact.omgc" | tr -d ' ')
FRAME_BYTES=$(wc -c < "$T/component-max.rfn" | tr -d ' ')
ELAPSED=$(( $(date +%s) - STARTED ))
echo "OMGRFN5 responsibility 1: exact source+harness OMGCOMP custody, v5 checked frame adds/EOF, cross-version rejection, opaque joins/claims, exact component and whole-frame teeth passed below Delta"
echo "  source=${SOURCE_BYTES}B procedures=${PROCEDURES}/128 locals=${MAX_LOCALS}/32 tape=${TAPE_BYTES}/262140B bc-self=${BC1_TAPE}B OMGCOMP=${OMGCOMP_BYTES}B component-frame=${FRAME_BYTES}B elapsed=${ELAPSED}s"
