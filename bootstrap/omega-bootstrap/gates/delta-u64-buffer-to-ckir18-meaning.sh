#!/usr/bin/env sh
# Rust-free persisted-Beta/Gamma meaning gate for OMGRSWA10/OMGLOWJ19.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "u64-buffer meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "u64-buffer meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "u64-buffer meaning: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-u64-buffer-resolve.alp
LOWERER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-u64-buffer-to-ckir.alp
SOURCE_FIXTURE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/omgrsw10_u64_buffer_fixture.py
LOWERING_FIXTURE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-u64-buffer-to-ckir18-fixture.py
CKIR_REFERENCE=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v18_reference.py
RUNNER=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-ckir4-meaning-runner.py
DECODER=$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py
for REQUIRED in "$RESOLVER" "$LOWERER" "$SOURCE_FIXTURE" "$LOWERING_FIXTURE" \
                "$CKIR_REFERENCE" "$RUNNER" "$DECODER"; do
  [ -f "$REQUIRED" ] || { echo "u64-buffer meaning: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "u64-buffer meaning FAIL - Beta compiler artifact" >&2
  exit 1
}
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" || {
  echo "u64-buffer meaning FAIL - omega2gamma build" >&2; exit 1;
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "u64-buffer meaning FAIL - Gamma interpreter build" >&2; exit 1;
}

# Each focused producer is elaborated independently.  This is deliberately not
# evidence from the historical resolver/lowerer monoliths.
python3 -B "$RUNNER" elaborate "$T/elaborate.exe" "$RESOLVER" \
  "$T/resolver.gamma" "$T/timings.tsv" "u64-buffer resolver meaning" 60 2200000
python3 -B "$RUNNER" elaborate "$T/elaborate.exe" "$LOWERER" \
  "$T/lowerer.gamma" "$T/timings.tsv" "u64-buffer lowerer meaning" 60 1800000

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null

PYTHONPATH=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES \
python3 -B - "$SOURCE_FIXTURE" "$LOWERING_FIXTURE" "$T" <<'PY'
import importlib.util
import struct
import sys
from pathlib import Path

source_path, lowering_path, output = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])

def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module

source = load("u64_source_fixture", source_path)
lowering = load("u64_lowering_fixture", lowering_path)
canonical = source.encode_compilation(source.CANONICAL)
semantic = source.encode_compilation(
    source.CANONICAL.replace("self.length + 1", "self.length + 2", 1))
resource = source.encode_compilation(
    source.CANONICAL.replace("last_retained", "r" * 65, 1))
for label, contents in (("resolver-canonical", canonical),
                        ("resolver-semantic-251", semantic),
                        ("resolver-resource-252", resource)):
    (output / f"{label}.input").write_bytes(contents)

# The witness is filled after the native resolver runs.  Persist the canonical
# compilation and helpers needed by the second fixture phase.
(output / "canonical.omgc").write_bytes(canonical)
PY

native_case() { # executable label expected
  CASE_EXECUTABLE=$1 CASE_LABEL=$2 CASE_EXPECTED=$3
  set +e
  "$CASE_EXECUTABLE" < "$T/$CASE_LABEL.input" > "$T/$CASE_LABEL.expected"
  CASE_STATUS=$?
  set -e
  [ "$CASE_STATUS" -eq "$CASE_EXPECTED" ] || {
    echo "u64-buffer meaning FAIL - $CASE_LABEL native status $CASE_STATUS expected $CASE_EXPECTED" >&2
    exit 1
  }
  if [ "$CASE_EXPECTED" -ne 0 ] && [ -s "$T/$CASE_LABEL.expected" ]; then
    echo "u64-buffer meaning FAIL - $CASE_LABEL native rejection published bytes" >&2
    exit 1
  fi
}
native_case "$T/resolver.native" resolver-canonical 0
native_case "$T/resolver.native" resolver-semantic-251 251
native_case "$T/resolver.native" resolver-resource-252 252

PYTHONPATH=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES \
python3 -B - "$LOWERING_FIXTURE" "$T" <<'PY'
import importlib.util
import struct
import sys
from pathlib import Path

fixture_path, output = Path(sys.argv[1]), Path(sys.argv[2])
spec = importlib.util.spec_from_file_location("u64_lowering_fixture", fixture_path)
fixture = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(fixture)
compilation = (output / "canonical.omgc").read_bytes()
witness = (output / "resolver-canonical.expected").read_bytes()
canonical = fixture.pack(compilation, witness)
semantic = fixture.pack(compilation, witness, selector=9)
resource = bytearray(canonical)
struct.pack_into("<I", resource, 20, 267_281)
for label, contents in (("lowerer-canonical", canonical),
                        ("lowerer-semantic-251", semantic),
                        ("lowerer-resource-252", bytes(resource))):
    (output / f"{label}.input").write_bytes(contents)
PY

native_case "$T/lowerer.native" lowerer-canonical 0
native_case "$T/lowerer.native" lowerer-semantic-251 251
native_case "$T/lowerer.native" lowerer-resource-252 252
[ "$(python3 -B "$CKIR_REFERENCE" run "$T/lowerer-canonical.expected")" = 70 ] || {
  echo "u64-buffer meaning FAIL - canonical CKIR18 result is not 70" >&2
  exit 1
}

launch_gamma() { # producer label
  CASE_PRODUCER=$1 CASE_LABEL=$2
  python3 -B "$RUNNER" run "$T/interp.exe" "$T/$CASE_PRODUCER.gamma" \
    "$T/$CASE_LABEL.input" "$T/$CASE_LABEL.observation" "$T/timings.tsv" \
    "u64-buffer meaning $CASE_LABEL" 240
}
check_gamma() { # label expected
  CASE_LABEL=$1 CASE_EXPECTED=$2
  CASE_STATUS=$(python3 -B "$DECODER" "$T/$CASE_LABEL.observation" "$T/$CASE_LABEL.stdout")
  [ "$CASE_STATUS" -eq "$CASE_EXPECTED" ] || {
    echo "u64-buffer meaning FAIL - $CASE_LABEL Gamma status $CASE_STATUS expected $CASE_EXPECTED" >&2
    exit 1
  }
  cmp "$T/$CASE_LABEL.stdout" "$T/$CASE_LABEL.expected" >/dev/null || {
    echo "u64-buffer meaning FAIL - $CASE_LABEL Gamma publication differs" >&2
    exit 1
  }
}

launch_gamma resolver resolver-canonical & P1=$!
launch_gamma resolver resolver-semantic-251 & P2=$!
launch_gamma resolver resolver-resource-252 & P3=$!
launch_gamma lowerer lowerer-canonical & P4=$!
launch_gamma lowerer lowerer-semantic-251 & P5=$!
launch_gamma lowerer lowerer-resource-252 & P6=$!
set +e
wait "$P1"; W1=$?
wait "$P2"; W2=$?
wait "$P3"; W3=$?
wait "$P4"; W4=$?
wait "$P5"; W5=$?
wait "$P6"; W6=$?
set -e
[ "$W1" -eq 0 ] && [ "$W2" -eq 0 ] && [ "$W3" -eq 0 ] && \
  [ "$W4" -eq 0 ] && [ "$W5" -eq 0 ] && [ "$W6" -eq 0 ] || {
  echo "u64-buffer meaning FAIL - Gamma children $W1/$W2/$W3/$W4/$W5/$W6" >&2
  exit 1
}
check_gamma resolver-canonical 0
check_gamma resolver-semantic-251 251
check_gamma resolver-resource-252 252
check_gamma lowerer-canonical 0
check_gamma lowerer-semantic-251 251
check_gamma lowerer-resource-252 252

python3 - "$T/timings.tsv" "$T/resolver-canonical.expected" \
              "$T/lowerer-canonical.expected" <<'PY'
from pathlib import Path
import sys

rows = []
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    seconds, size, label = line.split("\t", 2)
    rows.append(f"{label}={float(seconds):.2f}s/{size}B")
print("u64-buffer meaning: OMGRSWA10 and OMGLOWJ19 canonical 0 plus "
      "semantic 251/resource 252 no-publication observations PASS through "
      "persisted-Beta Gamma; " + " ".join(rows)
      + f" OMGRSWA={Path(sys.argv[2]).stat().st_size}B"
      + f" CKIR18={Path(sys.argv[3]).stat().st_size}B")
PY
