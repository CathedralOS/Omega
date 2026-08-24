#!/usr/bin/env sh
# Focused lower-rooted OMGRFN2 layer-1 framing and OMGCOMP custody gate.
set -eu

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
  *) echo "OMGRFN2 layer 1: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN2 layer 1: skipped ($TOOL absent)"
    exit 0
  }
done

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2-frame-omgcomp-custody.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2_bundle.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
for REQUIRED in "$CHECKER" "$PACKER" "$FIXTURE"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN2 layer 1: missing $REQUIRED" >&2; exit 1; }
done

PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$CHECKER")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN2 layer 1: persisted Beta checker exceeds 128 procedures ($PROCEDURES)" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null

build_checker() {
  NAME=$1
  SUFFIX=$2
  cp "$CHECKER" "$T/$NAME.beta"
  cat "$SUFFIX" >> "$T/$NAME.beta"
  "$BC" < "$T/$NAME.beta" > "$T/$NAME.asm"
  "$ASM" < "$T/$NAME.asm" > "$T/$NAME.tape"
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME" >/dev/null 2>&1
}

printf '\nproc main() { return omgrfn2_layer1_check() }\n' > "$T/main.beta"
build_checker check "$T/main.beta"

python3 "$FIXTURE" build "$T/canonical"
OMGCOMP="$T/canonical/compilation-envelope.bin"
printf 'opaque-OMGRSW1' > "$T/witness"
printf 'opaque-CKIR1' > "$T/ckir"
printf 'opaque-ELF' > "$T/elf"
: > "$T/empty"
python3 "$PACKER" "$OMGCOMP" "$T/witness" "$T/ckir" "$T/elf" \
  --result 70 > "$T/entry.rfn"
python3 "$PACKER" "$OMGCOMP" "$T/witness" "$T/ckir" "$T/empty" \
  --library > "$T/library.rfn"

# Independently derive the canonical source-ID -> nested-bundle content
# projection and make the persisted checker expose every exact extent.
python3 - "$OMGCOMP" "$T/extent-main.beta" <<'PY'
from pathlib import Path
import struct
import sys

data = Path(sys.argv[1]).read_bytes()
fields = struct.unpack_from("<8sHHHH12I", data)
bundle_length, package_count, source_count = fields[6], fields[9], fields[10]
bundle_at = len(data) - bundle_length
cursor = bundle_at + 16
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

lines = ["", "proc main() {", "    let status = omgrfn2_layer1_check()"]
lines += ["    to failed when (status != 0)", "    to span_0"]
for source_id, (start, length) in enumerate(expected):
    lines += [
        f"    state span_{source_id} {{",
        f"        to bad when (omgrfn2_source_content_start({source_id}) != {start})",
        f"        to length_{source_id}",
        "    }",
        f"    state length_{source_id} {{",
        f"        to bad when (omgrfn2_source_content_length({source_id}) != {length})",
        f"        to span_{source_id + 1}" if source_id + 1 < len(expected) else "        to success",
        "    }",
    ]
lines += [
    "    state failed { return status }",
    "    state bad { return 251 }",
    "    state success { return 0 }",
    "}",
]
Path(sys.argv[2]).write_text("\n".join(lines) + "\n", encoding="ascii")
PY
build_checker extent-check "$T/extent-main.beta"

observe() {
  EXPECTED=$1
  INPUT=$2
  LABEL=$3
  set +e
  "$T/check" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN2 layer 1: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN2 layer 1: $LABEL published stdout" >&2
    exit 1
  }
}

observe_with() {
  EXE=$1
  EXPECTED=$2
  INPUT=$3
  LABEL=$4
  set +e
  "$EXE" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN2 layer 1: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN2 layer 1: $LABEL published stdout" >&2
    exit 1
  }
}

observe 0 "$T/entry.rfn" "canonical entry frame"
observe 0 "$T/library.rfn" "canonical library frame"
observe_with "$T/extent-check" 0 "$T/entry.rfn" "source-ID content extents"

python3 - "$T/entry.rfn" "$T/cases" <<'PY'
from pathlib import Path
import struct
import sys

canonical = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
out.mkdir()

def put(name, value):
    (out / name).write_bytes(value)

def changed(at, value):
    value_out = bytearray(canonical)
    value_out[at] = value
    return bytes(value_out)

def u32(at, value):
    value_out = bytearray(canonical)
    struct.pack_into("<I", value_out, at, value)
    return bytes(value_out)

put("bad-magic", changed(0, ord("X")))
put("bad-version", u32(8, 1))
put("bad-flags", u32(12, 2))
put("bad-exit", u32(36, 71))
put("truncated", canonical[:-1])
put("trailing", canonical + b"\0")

# Declared component teeth select exhaustion before exact-frame mismatch.
put("over-omgcomp", u32(16, 267281))
put("over-witness", u32(20, 524289))
put("over-ckir", u32(24, 2260041))
put("over-elf", u32(28, 1052673))

omgcomp_length = struct.unpack_from("<I", canonical, 16)[0]
omgcomp_at = 40
omgcomp = canonical[omgcomp_at:omgcomp_at + omgcomp_length]
header = struct.unpack_from("<8sHHHH12I", omgcomp)
bundle_length, package_count, source_count = header[6], header[9], header[10]
source_table = 64 + package_count * 48
alias_table = source_table + source_count * 20
bundle_at = len(omgcomp) - bundle_length

bad_comp_magic = bytearray(canonical)
bad_comp_magic[omgcomp_at] ^= 1
put("bad-omgcomp-magic", bytes(bad_comp_magic))
bad_bundle_magic = bytearray(canonical)
bad_bundle_magic[omgcomp_at + bundle_at] ^= 1
put("bad-bundle-magic", bytes(bad_bundle_magic))
bad_bundle_count = bytearray(canonical)
struct.pack_into("<I", bad_bundle_count, omgcomp_at + bundle_at + 12, source_count + 1)
put("bad-bundle-count", bytes(bad_bundle_count))
bad_alias = bytearray(canonical)
requester = struct.unpack_from("<I", omgcomp, alias_table)[0]
struct.pack_into("<I", bad_alias, omgcomp_at + alias_table + 8, requester)
put("bad-alias-graph", bytes(bad_alias))

for name, at, value in (
    ("over-package-count", 32, 17),
    ("over-source-count", 36, 17),
    ("over-alias-count", 40, 33),
    ("over-string-count", 28, 65),
    ("over-bundle-length", 20, 263313),
):
    value_out = bytearray(canonical)
    struct.pack_into("<I", value_out, omgcomp_at + at, value)
    put(name, bytes(value_out))

# Opaque components and well-formed claims remain deliberately unauthoritative.
for name, at in (
    ("changed-witness", 40 + omgcomp_length),
    ("changed-ckir", 40 + omgcomp_length + struct.unpack_from("<I", canonical, 20)[0]),
    ("changed-elf", 40 + omgcomp_length + struct.unpack_from("<I", canonical, 20)[0]
                    + struct.unpack_from("<I", canonical, 24)[0]),
):
    put(name, changed(at, canonical[at] ^ 1))
claims = bytearray(canonical)
struct.pack_into("<II", claims, 32, 71, 71)
put("changed-valid-claims", bytes(claims))

# Source contents are structurally opaque but their exact containing extent is
# retained. Change one byte without changing framing to exercise that boundary.
cursor = bundle_at + 16
label_length, content_length = struct.unpack_from("<II", omgcomp, cursor)
assert content_length > 0
content_at = omgcomp_at + cursor + 8 + label_length
put("changed-source-content", changed(content_at, canonical[content_at] ^ 1))
PY

for CASE in bad-magic bad-version bad-flags bad-exit truncated trailing \
  bad-omgcomp-magic bad-bundle-magic bad-bundle-count bad-alias-graph; do
  observe 251 "$T/cases/$CASE" "$CASE"
done
for CASE in over-omgcomp over-witness over-ckir over-elf over-package-count \
  over-source-count over-alias-count over-string-count over-bundle-length; do
  observe 252 "$T/cases/$CASE" "$CASE"
done
for CASE in changed-witness changed-ckir changed-elf changed-valid-claims \
  changed-source-content; do
  observe 0 "$T/cases/$CASE" "$CASE"
done

python3 - "$T/oversized" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_bytes(b"\0" * 4104321)
PY
observe 252 "$T/oversized" "whole-frame overflow"

echo "OMGRFN2 layer 1: exact v2 framing, full OMGCOMP structure/content extents, opaque components, EOF, and 0/251/252 teeth passed below Delta"
