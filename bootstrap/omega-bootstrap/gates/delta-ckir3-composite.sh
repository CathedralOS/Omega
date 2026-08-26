#!/usr/bin/env sh
# Exact OMGLOW3 -> CKIR3 -> Linux x86-64 ELF composition for the focused
# constant-aggregate tranche. Component gates own exhaustive local matrices;
# this gate owns native/self producer-backend interchangeability, independent
# CKIR result and exact ELF reconstruction, and valid cross-pair rejection.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "CKIR3 composite: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "CKIR3 composite: skipped (compiler construction requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign rg; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "CKIR3 composite: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
PRODUCER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir3.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v3-to-elf.alp"
FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-frame.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py"
IR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v3_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v3_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
FIXTURES="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir3-constant-aggregates"
UNICODE="$OMEGA_REPO_ROOT/source/compiler/omega/psi/generated/unicode_tables.omg"
GENERATED_CUSTODY="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/generated_source_custody.py"
GENERATED_RECIPE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/generated-source-custody/unicode-tables.recipe.json"
for REQUIRED in "$RESOLVER" "$PRODUCER" "$BACKEND" "$FRAME" "$FIXTURE" \
  "$IR_REFERENCE" "$ELF_REFERENCE" "$LOWERMACHINE" "$UNICODE" \
  "$GENERATED_CUSTODY" "$GENERATED_RECIPE"; do
  [ -f "$REQUIRED" ] || {
    echo "CKIR3 composite: required input absent: $REQUIRED" >&2
    exit 1
  }
done
for SOURCE in "$PRODUCER" "$BACKEND"; do
  COUNT=$(rg -c '^machine ' "$SOURCE")
  [ "$COUNT" -lt 128 ] || {
    echo "CKIR3 composite: $(basename "$SOURCE") exceeds Delta machine ceiling ($COUNT)" >&2
    exit 1
  }
done

# The bridge still consumes the committed ordinary source below. This
# preflight proves that those exact bytes are the deterministic result of the
# sealed recipe before any compiler artifact is constructed.
python3 -B "$GENERATED_CUSTODY" reproduce "$GENERATED_RECIPE"

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(python3 -c 'import time; print(time.time())')

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER" "$T/producer.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null

build_self() { # label source
  "$T/lowermachine" < "$2" > "$T/$1.self.s"
  clang -arch arm64 -o "$T/$1.self" "$T/$1.self.s"
  codesign -f -s - "$T/$1.self" >/dev/null 2>&1
}
build_self producer "$PRODUCER"
build_self backend "$BACKEND"
BUILT=$(python3 -c 'import time; print(time.time())')

run_expect() { # executable input status output label
  set +e
  "$1" < "$2" > "$4" 2> "$4.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$3" ] || {
    echo "CKIR3 composite: $5 returned $ACTUAL, expected $3" >&2
    sed -n '1,20p' "$4.stderr" >&2
    exit 1
  }
  if [ "$3" -ne 0 ] && [ -s "$4" ]; then
    echo "CKIR3 composite: $5 published bytes on rejection" >&2
    exit 1
  fi
}

prepare() { # label owner machine source...
  LABEL=$1 OWNER=$2 MACHINE=$3
  shift 3
  python3 -B "$FIXTURE" build "$T/$LABEL.omgc" "$OWNER" "$MACHINE" "$@"
  run_expect "$T/resolver.native" "$T/$LABEL.omgc" 0 "$T/$LABEL.omgrsw1" "$LABEL resolver"
  python3 -B "$FRAME" pack "$T/$LABEL.omgc" "$T/$LABEL.omgrsw1" > "$T/$LABEL.omglow3"
  python3 -B "$FRAME" verify "$T/$LABEL.omglow3"
}

compose() { # label expected-result
  LABEL=$1 EXPECTED=$2
  run_expect "$T/producer.native" "$T/$LABEL.omglow3" 0 "$T/$LABEL.native.ckir3" "$LABEL native producer"
  run_expect "$T/producer.self" "$T/$LABEL.omglow3" 0 "$T/$LABEL.self.ckir3" "$LABEL self producer"
  cmp "$T/$LABEL.native.ckir3" "$T/$LABEL.self.ckir3" >/dev/null
  python3 -B "$IR_REFERENCE" validate "$T/$LABEL.native.ckir3" >/dev/null
  [ "$(python3 -B "$IR_REFERENCE" run "$T/$LABEL.native.ckir3")" = "$EXPECTED" ] || {
    echo "CKIR3 composite: $LABEL independent CKIR result mismatch" >&2
    exit 1
  }
  for PRODUCER_KIND in native self; do
    for BACKEND_KIND in native self; do
      run_expect "$T/backend.$BACKEND_KIND" "$T/$LABEL.$PRODUCER_KIND.ckir3" 0 \
        "$T/$LABEL.$PRODUCER_KIND-$BACKEND_KIND.elf" \
        "$LABEL $PRODUCER_KIND producer/$BACKEND_KIND backend"
    done
  done
  for PAIR in native-self self-native self-self; do
    cmp "$T/$LABEL.native-native.elf" "$T/$LABEL.$PAIR.elf" >/dev/null
  done
  python3 -B "$ELF_REFERENCE" check "$T/$LABEL.native.ckir3" \
    "$T/$LABEL.native-native.elf" >/dev/null
}

python3 -B - "$FIXTURES/guardless-transition.omg" "$T/guardless-71.omg" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_text(encoding="utf-8")
old = "state pass(&mut self) { 70 }"
if source.count(old) != 1:
    raise SystemExit("guardless result anchor")
Path(sys.argv[2]).write_text(source.replace(old, "state pass(&mut self) { 71 }"), encoding="utf-8")
PY

prepare guardless-70 GuardlessProbe run "$FIXTURES/guardless-transition.omg"
prepare guardless-71 GuardlessProbe run "$T/guardless-71.omg"
prepare cyclic CustodyCycle run "$FIXTURES/cyclic-range-custody.omg"
prepare renamed AggregateProbe run "$FIXTURES/renamed-reordered-nested.omg"
prepare unicode UnicodeTables bootstrap_constant_aggregate_probe \
  "$UNICODE" "$FIXTURES/unicode-harness.omg"

compose guardless-70 70
compose guardless-71 71
compose cyclic 70
compose renamed 70
compose unicode 70

[ "$(wc -c < "$T/cyclic.native.ckir3" | tr -d ' ')" -eq 2564 ]
[ "$(wc -c < "$T/unicode.native.ckir3" | tr -d ' ')" -eq 94172 ]
[ "$(wc -c < "$T/cyclic.native-native.elf" | tr -d ' ')" -eq 12288 ]
[ "$(wc -c < "$T/unicode.native-native.elf" | tr -d ' ')" -eq 24576 ]

# Repeated publication is exact, and nearby valid programs cannot exchange
# artifacts even when they share the same projected process exit byte class.
run_expect "$T/backend.native" "$T/unicode.native.ckir3" 0 \
  "$T/unicode.repeat.elf" "Unicode repeat"
cmp "$T/unicode.native-native.elf" "$T/unicode.repeat.elf" >/dev/null
for CROSS in \
  "guardless-70.native.ckir3 guardless-71.native-native.elf" \
  "guardless-71.native.ckir3 guardless-70.native-native.elf" \
  "cyclic.native.ckir3 unicode.native-native.elf" \
  "unicode.native.ckir3 cyclic.native-native.elf"; do
  set -- $CROSS
  if python3 -B "$ELF_REFERENCE" check "$T/$1" "$T/$2" \
      > "$T/cross.out" 2> "$T/cross.stderr"; then
    echo "CKIR3 composite: valid-but-mismatched $1/$2 accepted" >&2
    exit 1
  fi
done

# Representative producer and backend status seams. Exhaustive mutations and
# adjacent resources remain in their focused component gates.
prepare source-251 WrongFieldProbe run "$FIXTURES/negative-wrong-field.omg"
for KIND in native self; do
  run_expect "$T/producer.$KIND" "$T/source-251.omglow3" 251 \
    "$T/source-251.$KIND.out" "$KIND producer semantic rejection"
done
python3 -B - "$T/source-252.omglow3" <<'PY'
from pathlib import Path
import struct
import sys

comp, witness = 267_281, 0
total = 32 + comp + witness
raw = struct.pack("<8sHHHH4I", b"OMGLOW3\0", 3, 0, 0, 32, total, comp, witness, 0)
Path(sys.argv[1]).write_bytes(raw + bytes(comp + witness))
PY
for KIND in native self; do
  run_expect "$T/producer.$KIND" "$T/source-252.omglow3" 252 \
    "$T/source-252.$KIND.out" "$KIND producer resource rejection"
done
python3 -B - "$T/cyclic.native.ckir3" "$T/backend-251.ckir3" "$T/backend-252.ckir3" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
semantic = bytearray(source)
struct.pack_into("<H", semantic, 8, 2)
Path(sys.argv[2]).write_bytes(semantic)
resource = bytearray(source)
struct.pack_into("<I", resource, 72, 8193)
Path(sys.argv[3]).write_bytes(resource)
PY
for KIND in native self; do
  run_expect "$T/backend.$KIND" "$T/backend-251.ckir3" 251 \
    "$T/backend-251.$KIND.out" "$KIND backend semantic rejection"
  run_expect "$T/backend.$KIND" "$T/backend-252.ckir3" 252 \
    "$T/backend-252.$KIND.out" "$KIND backend resource rejection"
done

FINISHED=$(python3 -c 'import time; print(time.time())')
python3 - "$STARTED" "$BUILT" "$FINISHED" <<'PY'
import sys

started, built, finished = map(float, sys.argv[1:])
print(
    "CKIR3 composite: exact native/self/mixed CKIR3 and ELF, independent "
    "results 70/71 including renamed/reordered nesting, complete ELF "
    "reconstruction, valid cross-pairs, and "
    f"representative 251/252 seams passed; build {built-started:.2f}s, "
    f"compose {finished-built:.2f}s, total {finished-started:.2f}s"
)
PY
