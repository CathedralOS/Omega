#!/usr/bin/env sh
# Exact two-package OMGCOMP -> OMGRSW1 -> OMGLOW1 -> CKIR1 -> limited ELF.
# The component gates own exhaustive local matrices; this gate owns their
# byte-exact composition, cross-built agreement, and representative fail-closed
# seams.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "two-package composite: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "two-package composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "two-package composite: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-to-elf.alp"
PRODUCER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-source-custody-check.alp"
FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
MUTATIONS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/resolved_to_ckir_mutations.py"
CKIR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for REQUIRED in "$RESOLVER" "$LOWERER" "$BACKEND" "$PRODUCER" "$FRAME" \
  "$FIXTURE" "$MUTATIONS" "$CKIR_REFERENCE" "$ELF_REFERENCE" "$LOWERMACHINE"; do
  [ -f "$REQUIRED" ] || {
    echo "two-package composite: missing $REQUIRED" >&2
    exit 1
  }
done

for SOURCE in "$RESOLVER" "$LOWERER" "$BACKEND"; do
  MACHINE_COUNT=$(awk '/^machine / { count += 1 } END { print count + 0 }' "$SOURCE")
  [ "$MACHINE_COUNT" -le 128 ] || {
    echo "two-package composite: $(basename "$SOURCE") exceeds Delta machine ceiling ($MACHINE_COUNT)" >&2
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
DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER" "$T/producer.native" >/dev/null

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
    echo "two-package composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,20p' "$OUTPUT.stderr" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$OUTPUT" ]; then
    echo "two-package composite: $LABEL published bytes on rejection" >&2
    exit 1
  fi
)

python3 "$FIXTURE" build "$T/canonical"
OMGCOMP="$T/canonical/compilation-envelope.bin"
run_expect "$T/producer.native" "$T/canonical/reference.bundle" 0 \
  "$T/reference.ckir" "frozen CKIR reference"

pipeline() (
  LABEL=$1
  RESOLVER_EXE=$2
  LOWERER_EXE=$3
  BACKEND_EXE=$4
  run_expect "$RESOLVER_EXE" "$OMGCOMP" 0 "$T/$LABEL.omgrsw1" "$LABEL resolver"
  python3 "$FRAME" pack "$OMGCOMP" "$T/$LABEL.omgrsw1" > "$T/$LABEL.omglow"
  python3 "$FRAME" verify "$T/$LABEL.omglow"
  run_expect "$LOWERER_EXE" "$T/$LABEL.omglow" 0 "$T/$LABEL.ckir" "$LABEL lowerer"
  run_expect "$BACKEND_EXE" "$T/$LABEL.ckir" 0 "$T/$LABEL.elf" "$LABEL backend"
)

pipeline native "$T/resolver.native" "$T/lowerer.native" "$T/backend.native"
pipeline self "$T/resolver.self" "$T/lowerer.self" "$T/backend.self"
pipeline native-self-native "$T/resolver.native" "$T/lowerer.self" "$T/backend.native"
pipeline self-native-self "$T/resolver.self" "$T/lowerer.native" "$T/backend.self"

for CASE in self native-self-native self-native-self; do
  cmp "$T/native.omgrsw1" "$T/$CASE.omgrsw1" >/dev/null
  cmp "$T/native.ckir" "$T/$CASE.ckir" >/dev/null
  cmp "$T/native.elf" "$T/$CASE.elf" >/dev/null
done
cmp "$T/native.ckir" "$T/reference.ckir" >/dev/null
python3 "$FIXTURE" check-pair "$T/reference.ckir" "$T/native.ckir"
python3 "$CKIR_REFERENCE" run "$T/native.ckir" > "$T/product-status"
cmp "$T/canonical/expected-observation.txt" "$T/product-status" >/dev/null
python3 "$ELF_REFERENCE" mutation-sweep "$T/native.ckir" "$T/native.elf"

# A second valid source/witness/artifact gives relation-level cross-pairs rather
# than relying only on malformed bytes.
python3 "$MUTATIONS" parameter-envelope "$T/parameter.omgc"
run_expect "$T/resolver.native" "$T/parameter.omgc" 0 \
  "$T/parameter.omgrsw1" "parameter resolver"
python3 "$FRAME" pack "$T/parameter.omgc" "$T/parameter.omgrsw1" > "$T/parameter.omglow"
run_expect "$T/lowerer.native" "$T/parameter.omglow" 0 \
  "$T/parameter.ckir" "parameter lowerer"
run_expect "$T/backend.native" "$T/parameter.ckir" 0 \
  "$T/parameter.elf" "parameter backend"
python3 "$ELF_REFERENCE" check "$T/parameter.ckir" "$T/parameter.elf"

python3 "$FRAME" pack "$OMGCOMP" "$T/parameter.omgrsw1" > "$T/canonical-parameter.omglow"
python3 "$FRAME" pack "$T/parameter.omgc" "$T/native.omgrsw1" > "$T/parameter-canonical.omglow"
run_expect "$T/lowerer.native" "$T/canonical-parameter.omglow" 251 \
  "$T/canonical-parameter.native.out" "native canonical/parameter cross-pair"
run_expect "$T/lowerer.self" "$T/canonical-parameter.omglow" 251 \
  "$T/canonical-parameter.self.out" "self canonical/parameter cross-pair"
run_expect "$T/lowerer.native" "$T/parameter-canonical.omglow" 251 \
  "$T/parameter-canonical.native.out" "native parameter/canonical cross-pair"

if python3 "$ELF_REFERENCE" check "$T/parameter.ckir" "$T/native.elf" \
  > "$T/mismatched-ckir-elf.out" 2> "$T/mismatched-ckir-elf.stderr"; then
  echo "two-package composite: valid-but-mismatched CKIR/ELF pair accepted" >&2
  exit 1
fi

# Representative checked failures at every executable seam. Each native and
# self-built consumer must preserve the exact status class and publish nothing.
python3 - "$OMGCOMP" "$T/resolver-251.omgc" "$T/resolver-252.omgc" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
malformed = bytearray(source)
malformed[0] ^= 1
Path(sys.argv[2]).write_bytes(malformed)
exhausted = bytearray(source)
struct.pack_into("<I", exhausted, 32, 17)
Path(sys.argv[3]).write_bytes(exhausted)
PY
for KIND in native self; do
  run_expect "$T/resolver.$KIND" "$T/resolver-251.omgc" 251 \
    "$T/resolver-$KIND-251.out" "$KIND resolver semantic rejection"
  run_expect "$T/resolver.$KIND" "$T/resolver-252.omgc" 252 \
    "$T/resolver-$KIND-252.out" "$KIND resolver resource exhaustion"
done

python3 "$MUTATIONS" build "$T/native.omglow" "$T/parameter.omglow" "$T/mutations"
for KIND in native self; do
  run_expect "$T/lowerer.$KIND" "$T/mutations/source-witness-body.omglow" 251 \
    "$T/lowerer-$KIND-251.out" "$KIND lowerer semantic rejection"
  run_expect "$T/lowerer.$KIND" "$T/mutations/witness-type-count-2049.omglow" 252 \
    "$T/lowerer-$KIND-252.out" "$KIND lowerer resource exhaustion"
done

python3 - "$T/native.ckir" "$T/backend-251.ckir" "$T/backend-252.ckir" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
malformed = bytearray(source)
malformed[0] ^= 1
Path(sys.argv[2]).write_bytes(malformed)
exhausted = bytearray(source)
struct.pack_into("<I", exhausted, 24, 8193)
Path(sys.argv[3]).write_bytes(exhausted)
PY
for KIND in native self; do
  run_expect "$T/backend.$KIND" "$T/backend-251.ckir" 251 \
    "$T/backend-$KIND-251.out" "$KIND backend semantic rejection"
  run_expect "$T/backend.$KIND" "$T/backend-252.ckir" 252 \
    "$T/backend-$KIND-252.out" "$KIND backend resource exhaustion"
done

echo "two-package composite: exact native/self/cross-built OMGRSW1, CKIR1, ELF, result 70, reconstruction, and representative 0/251/252 seams passed"
