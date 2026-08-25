#!/usr/bin/env sh
# Exact same-module/cross-source OMGCOMP -> OMGRSW1 -> OMGLOW2 -> CKIR2 ->
# Linux x86-64 ELF composition for the first attached-call DAG.  Component
# gates own exhaustive local matrices; this gate owns byte-exact composition,
# native/self-built interchangeability, and representative fail-closed seams.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "role-3 CKIR2 composite: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "role-3 CKIR2 composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "role-3 CKIR2 composite: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir2.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v2-to-elf.alp"
FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow2.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/role3_resolution_fixture.py"
CKIR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/ckir2_call_reference.py"
SEMANTICS_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v2_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v2_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for REQUIRED in "$RESOLVER" "$LOWERER" "$BACKEND" "$FRAME" "$FIXTURE" \
  "$CKIR_REFERENCE" "$SEMANTICS_REFERENCE" "$ELF_REFERENCE" "$LOWERMACHINE"; do
  [ -f "$REQUIRED" ] || {
    echo "role-3 CKIR2 composite: missing $REQUIRED" >&2
    exit 1
  }
done

for SOURCE in "$RESOLVER" "$LOWERER" "$BACKEND"; do
  MACHINE_COUNT=$(awk '/^machine / { count += 1 } END { print count + 0 }' "$SOURCE")
  [ "$MACHINE_COUNT" -le 128 ] || {
    echo "role-3 CKIR2 composite: $(basename "$SOURCE") exceeds Delta machine ceiling ($MACHINE_COUNT)" >&2
    exit 1
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null

build_self() (
  LABEL=$1
  SOURCE=$2
  "$T/lowermachine" < "$SOURCE" > "$T/$LABEL.self.s"
  clang -arch arm64 -o "$T/$LABEL.self" "$T/$LABEL.self.s"
  codesign -f -s - "$T/$LABEL.self" >/dev/null 2>&1
)
build_self resolver "$RESOLVER"
build_self lowerer "$LOWERER"
build_self backend "$BACKEND"

run_expect() (
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
    echo "role-3 CKIR2 composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,20p' "$OUTPUT.stderr" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$OUTPUT" ]; then
    echo "role-3 CKIR2 composite: $LABEL published bytes on rejection" >&2
    exit 1
  fi
)

python3 "$FIXTURE" build "$T/fixture"
OMGCOMP="$T/fixture/valid.omgc"

pipeline() (
  LABEL=$1
  RESOLVER_EXE=$2
  LOWERER_EXE=$3
  BACKEND_EXE=$4
  run_expect "$RESOLVER_EXE" "$OMGCOMP" 0 "$T/$LABEL.omgrsw1" "$LABEL resolver"
  python3 "$FRAME" pack "$OMGCOMP" "$T/$LABEL.omgrsw1" > "$T/$LABEL.omglow2"
  python3 "$FRAME" verify "$T/$LABEL.omglow2"
  run_expect "$LOWERER_EXE" "$T/$LABEL.omglow2" 0 "$T/$LABEL.ckir2" "$LABEL lowerer"
  run_expect "$BACKEND_EXE" "$T/$LABEL.ckir2" 0 "$T/$LABEL.elf" "$LABEL backend"
)

pipeline native "$T/resolver.native" "$T/lowerer.native" "$T/backend.native"
pipeline self "$T/resolver.self" "$T/lowerer.self" "$T/backend.self"
pipeline native-self-native "$T/resolver.native" "$T/lowerer.self" "$T/backend.native"
pipeline self-native-self "$T/resolver.self" "$T/lowerer.native" "$T/backend.self"
pipeline native-repeat "$T/resolver.native" "$T/lowerer.native" "$T/backend.native"

for CASE in self native-self-native self-native-self native-repeat; do
  cmp "$T/native.omgrsw1" "$T/$CASE.omgrsw1" >/dev/null
  cmp "$T/native.omglow2" "$T/$CASE.omglow2" >/dev/null
  cmp "$T/native.ckir2" "$T/$CASE.ckir2" >/dev/null
  cmp "$T/native.elf" "$T/$CASE.elf" >/dev/null
done

# Independent source/witness, checked-IR semantic, pinned-byte, and ELF
# reconstruction oracles jointly check the composed product.  The expected
# sizes make accidental empty/truncated agreement explicit at this boundary.
python3 "$FIXTURE" check "$OMGCOMP" "$T/native.omgrsw1" >/dev/null
python3 "$CKIR_REFERENCE" emit "$T/expected.ckir2"
cmp "$T/expected.ckir2" "$T/native.ckir2" >/dev/null
[ "$(wc -c < "$T/native.ckir2" | tr -d ' ')" -eq 1020 ]
[ "$(python3 "$CKIR_REFERENCE" check "$T/native.ckir2")" = 70 ]
python3 "$SEMANTICS_REFERENCE" validate "$T/native.ckir2" >/dev/null
[ "$(python3 "$SEMANTICS_REFERENCE" run "$T/native.ckir2")" = 70 ]
python3 "$ELF_REFERENCE" mutation-sweep "$T/native.ckir2" "$T/native.elf" >/dev/null
[ "$(wc -c < "$T/native.elf" | tr -d ' ')" -eq 8192 ]

# Representative checked failures at every executable phase.  Both the native
# and Delta-self-built consumers must retain status class and publish nothing.
python3 - "$OMGCOMP" "$T/resolver-251.omgc" "$T/resolver-252.omgc" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
malformed = bytearray(source)
malformed[0] ^= 1
Path(sys.argv[2]).write_bytes(malformed)
exhausted = bytearray(source)
struct.pack_into("<I", exhausted, 32, 17)  # package count > 16
Path(sys.argv[3]).write_bytes(exhausted)
PY
for KIND in native self; do
  run_expect "$T/resolver.$KIND" "$T/resolver-251.omgc" 251 \
    "$T/resolver-$KIND-251.out" "$KIND resolver semantic rejection"
  run_expect "$T/resolver.$KIND" "$T/resolver-252.omgc" 252 \
    "$T/resolver-$KIND-252.out" "$KIND resolver resource exhaustion"
done

python3 - "$T/native.omgrsw1" "$T/lowerer-251.omgrsw1" "$T/lowerer-252.omgrsw1" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
malformed = bytearray(source)
struct.pack_into("<I", malformed, 64, 3)  # witness root disagrees with OMGCOMP
Path(sys.argv[2]).write_bytes(malformed)
exhausted = bytearray(source)
struct.pack_into("<I", exhausted, 36, 2049)  # type count > 2048
Path(sys.argv[3]).write_bytes(exhausted)
PY
python3 "$FRAME" pack "$OMGCOMP" "$T/lowerer-251.omgrsw1" > "$T/lowerer-251.omglow2"
python3 "$FRAME" pack "$OMGCOMP" "$T/lowerer-252.omgrsw1" > "$T/lowerer-252.omglow2"
for KIND in native self; do
  run_expect "$T/lowerer.$KIND" "$T/lowerer-251.omglow2" 251 \
    "$T/lowerer-$KIND-251.out" "$KIND lowerer relation rejection"
  run_expect "$T/lowerer.$KIND" "$T/lowerer-252.omglow2" 252 \
    "$T/lowerer-$KIND-252.out" "$KIND lowerer resource exhaustion"
done

python3 - "$T/native.ckir2" "$T/backend-251.ckir2" "$T/backend-252.ckir2" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
malformed = bytearray(source)
struct.pack_into("<H", malformed, 8, 1)  # CKIR1 schema at CKIR2 boundary
Path(sys.argv[2]).write_bytes(malformed)
exhausted = bytearray(source)
struct.pack_into("<I", exhausted, 24 + 7 * 4, 32769)  # operation count > 32768
Path(sys.argv[3]).write_bytes(exhausted)
PY
for KIND in native self; do
  run_expect "$T/backend.$KIND" "$T/backend-251.ckir2" 251 \
    "$T/backend-$KIND-251.out" "$KIND backend semantic rejection"
  run_expect "$T/backend.$KIND" "$T/backend-252.ckir2" 252 \
    "$T/backend-$KIND-252.out" "$KIND backend resource exhaustion"
done

echo "role-3 CKIR2 composite: exact native/self/cross-built OMGRSW1, OMGLOW2, CKIR2, 8192-byte ELF, result 70, determinism, independent reconstruction, and phase-local 0/251/252 seams passed"
