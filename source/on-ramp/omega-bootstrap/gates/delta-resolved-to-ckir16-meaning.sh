#!/usr/bin/env sh
# Persisted-Beta/Gamma meaning probe for the focused OMGLOWH -> CKIR16 u64 lane.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "resolved-to-CKIR16 meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "resolved-to-CKIR16 meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolved-to-CKIR16 meaning: skipped ($TOOL absent)"
    exit 0
  }
done

LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir4.alp"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir16-fixture.py"
SOURCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir16-u64-less/general.omg"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v16_reference.py"
RUNNER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-ckir4-meaning-runner.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
for REQUIRED in "$LOWERER" "$RESOLVER" "$FIXTURE" "$SOURCE" "$REFERENCE" "$RUNNER" "$DECODER"; do
  [ -f "$REQUIRED" ] || { echo "resolved-to-CKIR16 meaning: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "resolved-to-CKIR16 meaning FAIL - Beta compiler artifact" >&2
  exit 1
}
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" || {
  echo "resolved-to-CKIR16 meaning FAIL - omega2gamma build" >&2; exit 1;
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "resolved-to-CKIR16 meaning FAIL - Gamma interpreter build" >&2; exit 1;
}

# Translate the actual complete lowerer once. All observations reuse this one
# persisted-Beta-produced Gamma program.
python3 -B "$RUNNER" elaborate "$T/elaborate.exe" "$LOWERER" \
  "$T/lowerer.gamma" "$T/timings.tsv" "resolved-to-CKIR16 meaning" 45 2900000

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null

PYTHONPATH="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" \
python3 -B - "$FIXTURE" "$SOURCE" "$T" <<'PY'
import importlib.util
import struct
import sys
from pathlib import Path

fixture_path, source_path, output = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
spec = importlib.util.spec_from_file_location("ckir16_fixture", fixture_path)
fixture = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(fixture)

canonical = source_path.read_text(encoding="ascii")
semantic = canonical.replace("false -> failed()", "false -> bounded(self.stored)", 1)
for label, source in (("canonical", canonical), ("semantic-251", semantic)):
    (output / f"{label}.omgc").write_bytes(fixture.encode_source(source))
PY

prepare() {
  LABEL=$1
  "$T/resolver.native" < "$T/$LABEL.omgc" > "$T/$LABEL.omgrsw"
  PYTHONPATH="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" \
    python3 -B - "$FIXTURE" "$T/$LABEL.omgc" "$T/$LABEL.omgrsw" "$T/$LABEL.omglow" <<'PY'
import importlib.util
import sys
from pathlib import Path

fixture_path, comp_path, witness_path, output_path = map(Path, sys.argv[1:])
spec = importlib.util.spec_from_file_location("ckir16_fixture", fixture_path)
fixture = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(fixture)
output_path.write_bytes(fixture.pack_lowering(comp_path.read_bytes(), witness_path.read_bytes()))
PY
}
prepare canonical
prepare semantic-251
cp "$T/canonical.omglow" "$T/resource-252.omglow"
python3 - "$T/resource-252.omglow" <<'PY'
from pathlib import Path
import struct
import sys

path = Path(sys.argv[1])
raw = bytearray(path.read_bytes())
struct.pack_into("<I", raw, 20, 267_281)
path.write_bytes(raw)
PY

: > "$T/empty.expected"
native_case() {
  LABEL=$1 EXPECTED=$2
  set +e
  "$T/lowerer.native" < "$T/$LABEL.omglow" > "$T/$LABEL.expected"
  STATUS=$?
  set -e
  [ "$STATUS" -eq "$EXPECTED" ] || {
    echo "resolved-to-CKIR16 meaning FAIL - $LABEL native status $STATUS, expected $EXPECTED" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$T/$LABEL.expected" ]; then
    echo "resolved-to-CKIR16 meaning FAIL - $LABEL native rejection published bytes" >&2
    exit 1
  fi
}
native_case canonical 0
native_case semantic-251 251
native_case resource-252 252
[ "$(python3 -B "$REFERENCE" run "$T/canonical.expected")" = 70 ] || {
  echo "resolved-to-CKIR16 meaning FAIL - canonical CKIR16 result is not 70" >&2
  exit 1
}

launch_gamma() {
  LABEL=$1
  python3 -B "$RUNNER" run "$T/interp.exe" "$T/lowerer.gamma" \
    "$T/$LABEL.omglow" "$T/$LABEL.observation" "$T/timings.tsv" \
    "resolved-to-CKIR16 meaning $LABEL" 180
}
check_gamma() {
  LABEL=$1 EXPECTED=$2
  STATUS=$(python3 -B "$DECODER" "$T/$LABEL.observation" "$T/$LABEL.stdout")
  [ "$STATUS" -eq "$EXPECTED" ] || {
    echo "resolved-to-CKIR16 meaning FAIL - $LABEL status $STATUS, expected $EXPECTED" >&2
    exit 1
  }
  cmp "$T/$LABEL.stdout" "$T/$LABEL.expected" >/dev/null || {
    echo "resolved-to-CKIR16 meaning FAIL - $LABEL publication differs" >&2
    exit 1
  }
}
launch_gamma canonical & CANONICAL_PID=$!
launch_gamma semantic-251 & SEMANTIC_PID=$!
launch_gamma resource-252 & RESOURCE_PID=$!
set +e
wait "$CANONICAL_PID"; CANONICAL_WAIT=$?
wait "$SEMANTIC_PID"; SEMANTIC_WAIT=$?
wait "$RESOURCE_PID"; RESOURCE_WAIT=$?
set -e
[ "$CANONICAL_WAIT" -eq 0 ] && [ "$SEMANTIC_WAIT" -eq 0 ] && \
  [ "$RESOURCE_WAIT" -eq 0 ] || {
  echo "resolved-to-CKIR16 meaning FAIL - Gamma child status canonical=$CANONICAL_WAIT semantic=$SEMANTIC_WAIT resource=$RESOURCE_WAIT" >&2
  exit 1
}
check_gamma canonical 0
check_gamma semantic-251 251
check_gamma resource-252 252

python3 - "$T/timings.tsv" "$T/canonical.expected" <<'PY'
from pathlib import Path
import sys

rows = []
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    seconds, size, label = line.split("\t", 2)
    rows.append(f"{label}={float(seconds):.2f}s/{size}B")
print("resolved-to-CKIR16 meaning: full-width direct u64 Less carries the "
      "borrow-bound true edge through storage/call/edge and returns 70; "
      "false-edge custody=251 and outer component capacity=252; exact "
      "publication through canonical Gamma; " + " ".join(rows)
      + f" CKIR16={Path(sys.argv[2]).stat().st_size}B")
PY
