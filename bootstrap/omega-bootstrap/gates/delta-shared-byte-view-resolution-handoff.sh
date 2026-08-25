#!/usr/bin/env sh
# Focused native/self gate for OMGRSW4 shared byte-view resolution.
set -e

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "shared byte view resolution: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "shared byte view resolution: skipped (requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "shared byte view resolution: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
CONTRACT="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/OMEGA_BOOTSTRAP_RESOLUTION_V4.md"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/shared_byte_view_resolution_fixture.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for FILE in "$RESOLVER" "$CONTRACT" "$FIXTURE" "$LOWERMACHINE"; do
  [ -f "$FILE" ] || { echo "shared byte view resolution: missing $FILE" >&2; exit 1; }
done

MACHINE_COUNT=$(awk '/^machine / { count += 1 } END { print count + 0 }' "$RESOLVER")
[ "$MACHINE_COUNT" -le 128 ] || {
  echo "shared byte view resolution: resolver exceeds Delta machine ceiling ($MACHINE_COUNT)" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
"$T/lowermachine" < "$RESOLVER" > "$T/resolver.self.s"
clang -arch arm64 -o "$T/resolver.self" "$T/resolver.self.s"
codesign -f -s - "$T/resolver.self" >/dev/null 2>&1

python3 "$FIXTURE" build "$T/fixtures"
python3 - "$T/fixtures/index.json" "$T/cases.tsv" <<'PY'
import json
import sys
rows = json.load(open(sys.argv[1], encoding="utf-8"))
with open(sys.argv[2], "w", encoding="utf-8") as output:
    for row in rows:
        output.write(f"{row['name']}\t{row['status']}\n")
PY

run_expect() {
  EXE=$1
  INPUT=$2
  EXPECTED=$3
  OUTPUT=$4
  LABEL=$5
  set +e
  "$EXE" < "$INPUT" > "$OUTPUT" 2> "$OUTPUT.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "shared byte view resolution: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,20p' "$OUTPUT.stderr" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$OUTPUT" ]; then
    echo "shared byte view resolution: $LABEL published bytes on rejection" >&2
    exit 1
  fi
}

while IFS="	" read -r NAME EXPECTED; do
  run_expect "$T/resolver.native" "$T/fixtures/$NAME.omgc" "$EXPECTED" "$T/native-$NAME.out" "native $NAME"
done < "$T/cases.tsv"

python3 "$FIXTURE" check "$T/fixtures/valid-v4.omgc" "$T/native-valid-v4.out"
python3 "$FIXTURE" check-magic "$T/native-legacy-v1.out" OMGRSW1
python3 "$FIXTURE" check-magic "$T/native-legacy-v2.out" OMGRSW2
python3 "$FIXTURE" check-magic "$T/native-legacy-v3.out" OMGRSW3
python3 "$FIXTURE" check-magic "$T/native-slice-before-sum.out" OMGRSW4
python3 "$FIXTURE" check-magic "$T/native-literal-before-state-slice.out" OMGRSW4
python3 "$FIXTURE" check-magic "$T/native-literal-empty.out" OMGRSW4
python3 "$FIXTURE" check-magic "$T/native-literal-32.out" OMGRSW4

for NAME in valid-v4 legacy-v1 legacy-v2 legacy-v3 slice-before-sum literal-before-state-slice literal-32; do
  run_expect "$T/resolver.self" "$T/fixtures/$NAME.omgc" 0 "$T/self-$NAME.out" "self-built $NAME"
  cmp "$T/native-$NAME.out" "$T/self-$NAME.out" >/dev/null || {
    echo "shared byte view resolution: native/self mismatch for $NAME" >&2
    exit 1
  }
done
for ROW in "slice-u32:251" "literal-escape:251" "literal-33:252"; do
  NAME=${ROW%:*}; EXPECTED=${ROW#*:}
  run_expect "$T/resolver.self" "$T/fixtures/$NAME.omgc" "$EXPECTED" "$T/self-$NAME.out" "self-built $NAME"
done

python3 - "$T/native-valid-v4.out" "$T/native-legacy-v3.out" "$T/mutations" <<'PY'
import struct
import sys
from pathlib import Path

raw = bytearray(Path(sys.argv[1]).read_bytes())
legacy = Path(sys.argv[2]).read_bytes()
out = Path(sys.argv[3]); out.mkdir()

def save(name, data): (out / f"{name}.omgrsw").write_bytes(data)

for name, offset, value in (
    ("magic", 6, 51), ("major", 8, 3), ("minor", 10, 1),
):
    changed = bytearray(raw); changed[offset] = value; save(name, changed)

words = struct.unpack_from("<17I", raw, 16)
sources, imports, bindings, declarations = words[1:5]
type_at = 84 + 36*sources + 48*imports + 28*bindings + 28*declarations
slice_at = type_at + 24*6
for name, offset, value in (
    ("kind", slice_at+4, 5), ("flags", slice_at+5, 1),
    ("payload0", slice_at+8, 4), ("payload1", slice_at+12, 1),
    ("low", slice_at+16, 1), ("high", slice_at+20, 1),
):
    changed = bytearray(raw)
    if offset in (slice_at+4, slice_at+5): changed[offset] = value
    else: struct.pack_into("<I", changed, offset, value)
    save(name, changed)

types, records, fields, machines = words[5], words[6], words[7], words[8]
machine_parameters, blocks = words[9], words[10]
block_parameters, sums, cases, payloads = words[11], words[12], words[13], words[14]
machine_parameter_at = (type_at + 24*types + 24*records + 24*fields
                        + 24*sums + 28*cases + 24*payloads + 40*machines)
block_at = machine_parameter_at + 24*machine_parameters
block_parameter_at = block_at + 40*blocks
for name, offset in (("machine-parameter-type", machine_parameter_at+12),
                     ("block-parameter-type", block_parameter_at+12)):
    changed = bytearray(raw); struct.pack_into("<I", changed, offset, 5); save(name, changed)

save("trailing", raw + b"\0")
save("v3-cross-pair", legacy)
PY

for INPUT in "$T/mutations"/*.omgrsw; do
  if python3 "$FIXTURE" check "$T/fixtures/valid-v4.omgc" "$INPUT" >/dev/null 2>&1; then
    echo "shared byte view resolution: independent reference accepted $(basename "$INPUT")" >&2
    exit 1
  fi
done

echo "shared byte view resolution: OMGRSW4 native/self/reference, least V1-V3, type/literal/version/resource controls passed"
