#!/usr/bin/env sh
# Fast focused CKIR4 -> deterministic Linux x86-64 ELF backend gate. It starts
# from independently handcrafted CKIR and does not claim source lowering.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "checked-IR-v4 backend: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v4 backend: skipped (compiler construction requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign rg; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v4 backend: skipped ($TOOL absent)"
    exit 0
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v4-to-elf.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-checked-ir-v4-fixture.py"
V3_FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-checked-ir-v3-backend-fixture.py"
IR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v4_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v4_reference.py"
V3_IR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v3_reference.py"
V3_ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v3_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for REQUIRED in "$BACKEND" "$FIXTURE" "$V3_FIXTURE" "$IR_REFERENCE" \
    "$ELF_REFERENCE" "$V3_IR_REFERENCE" "$V3_ELF_REFERENCE" "$LOWERMACHINE"; do
  [ -f "$REQUIRED" ] || {
    echo "checked-IR-v4 backend: required input absent: $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
GATE_STARTED=$(python3 -c 'import time; print(time.time())')
cleanup() {
  if [ "${OMEGA_KEEP_CKIR4_BACKEND_TEMP:-0}" = 1 ]; then
    echo "checked-IR-v4 backend: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT
: > "$T/timings.tsv"

# Count exactly the metadata bounded by the persisted Delta lowermachine.
python3 - "$BACKEND" "$T/metadata.tsv" <<'PY'
from pathlib import Path
import re
import sys

source = Path(sys.argv[1]).read_text(encoding="utf-8")
start = source.index("data Main {") + len("data Main {")
depth = 1
end = start
while depth:
    depth += (source[end] == "{") - (source[end] == "}")
    end += 1
body = re.sub(r"//.*", "", source[start:end - 1])
fields = 0
for statement in body.split(";"):
    if ":" in statement:
        names = statement.rsplit("\n", 1)[-1].split(":", 1)[0]
        fields += sum(bool(item.strip()) for item in names.split(","))
procedures = len(re.findall(r"^machine ", source, re.MULTILINE))
lets = len(re.findall(r"\blet\s+[A-Za-z_][A-Za-z0-9_]*\s*:", source))
state_parameters = sum(
    match.group(1).count(":")
    for match in re.finditer(
        r"\bstate\s+[A-Za-z_][A-Za-z0-9_]*\s*\(([^)]*)\)", source
    )
)
Path(sys.argv[2]).write_text(
    f"{procedures}\t{fields}\t{lets + state_parameters}\n", encoding="ascii"
)
PY
TAB=$(printf '\t')
IFS="$TAB" read -r PROCEDURES FIELDS LOCALS < "$T/metadata.tsv"
[ "$PROCEDURES" -lt 128 ] || {
  echo "checked-IR-v4 backend: procedures $PROCEDURES exceed Delta ceiling 127" >&2
  exit 1
}
[ "$FIELDS" -le 255 ] || {
  echo "checked-IR-v4 backend: fields $FIELDS exceed Delta ceiling 255" >&2
  exit 1
}
[ "$LOCALS" -le 32 ] || {
  echo "checked-IR-v4 backend: locals $LOCALS exceed Delta ceiling 32" >&2
  exit 1
}

# Every compiler-sized observation is timed and bounded independently. Expected
# rejections are successful observations, but may not publish partial output.
python3 - "$T/observe.py" <<'PY'
from pathlib import Path
import os
import signal
import subprocess
import sys
import time

Path(sys.argv[1]).write_text(r'''#!/usr/bin/env python3
from pathlib import Path
import os
import signal
import subprocess
import sys
import time

label, expected_text, timeout_text, input_name, output_name, timing_name, *command = sys.argv[1:]
expected = int(expected_text)
timeout = float(timeout_text)
started = time.monotonic()
with open(input_name, "rb") as source:
    process = subprocess.Popen(
        command, stdin=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        Path(output_name).write_bytes(stdout)
        Path(output_name + ".stderr").write_bytes(stderr)
        print(f"checked-IR-v4 backend FAIL - {label} exceeded {timeout:.0f}s", file=sys.stderr)
        raise SystemExit(1)
elapsed = time.monotonic() - started
Path(output_name).write_bytes(stdout)
Path(output_name + ".stderr").write_bytes(stderr)
with open(timing_name, "a", encoding="ascii") as timings:
    timings.write(f"{elapsed:.6f}\t{label}\n")
if process.returncode != expected:
    print(
        f"checked-IR-v4 backend FAIL - {label} status {process.returncode}, "
        f"expected {expected} ({elapsed:.2f}s)", file=sys.stderr,
    )
    if stderr:
        sys.stderr.buffer.write(stderr[-4096:])
    raise SystemExit(1)
''', encoding="utf-8")
PY

observe() { # label expected timeout stdin stdout command...
  OBS_LABEL=$1
  OBS_EXPECTED=$2
  OBS_TIMEOUT=$3
  OBS_INPUT=$4
  OBS_OUTPUT=$5
  shift 5
  python3 "$T/observe.py" "$OBS_LABEL" "$OBS_EXPECTED" "$OBS_TIMEOUT" \
    "$OBS_INPUT" "$OBS_OUTPUT" "$T/timings.tsv" "$@"
}

assert_empty() {
  [ ! -s "$1" ] || {
    echo "checked-IR-v4 backend FAIL - $2 published bytes on rejection" >&2
    exit 1
  }
}

observe build-cargo 0 120 /dev/null "$T/cargo.stdout" \
  cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
observe build-lowermachine 0 90 /dev/null "$T/lowermachine.stdout" \
  env DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine"
observe build-backend-native 0 90 /dev/null "$T/backend-native.stdout" \
  env DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native"
observe build-backend-self-source 0 120 "$BACKEND" "$T/backend.self.s" \
  "$T/lowermachine"
observe build-backend-self-link 0 90 /dev/null "$T/clang.stdout" \
  clang -arch arm64 -o "$T/backend.self" "$T/backend.self.s"
observe build-backend-self-sign 0 30 /dev/null "$T/codesign.stdout" \
  codesign -f -s - "$T/backend.self"

SELF_ASM_BYTES=$(wc -c < "$T/backend.self.s" | tr -d ' ')
[ "$SELF_ASM_BYTES" -le 1048576 ] || {
  echo "checked-IR-v4 backend: self asm $SELF_ASM_BYTES exceeds measured ceiling 1048576" >&2
  exit 1
}

observe generate-ckir4 0 30 /dev/null "$T/fixture.stdout" \
  python3 -B "$FIXTURE" emit "$T/cases"
observe generate-ckir3-regression 0 30 /dev/null "$T/v3-fixture.stdout" \
  python3 -B "$V3_FIXTURE" emit "$T/inherited.ckir3"

# Promote only the schema major. Independently reconstruct the original CKIR3
# artifact so byte identity also proves that inherited lowering did not drift.
PYTHONPATH="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" python3 - \
    "$T/inherited.ckir3" "$T/inherited.ckir4" "$T/inherited-v3-reference.elf" <<'PY'
from pathlib import Path
import struct
import sys

import checked_elf_v3_reference as elf3
import checked_ir_v3_reference as ir3

source = Path(sys.argv[1]).read_bytes()
Path(sys.argv[3]).write_bytes(elf3.Reconstructor(ir3.decode(source)).reconstruct())
promoted = bytearray(source)
struct.pack_into("<H", promoted, 8, 4)
Path(sys.argv[2]).write_bytes(promoted)
PY

CANONICAL="$T/cases/canonical.ckir4"
INHERITED="$T/inherited.ckir4"
[ "$(python3 -B "$IR_REFERENCE" run "$CANONICAL")" = 70 ] || {
  echo "checked-IR-v4 backend: canonical CKIR result is not 70" >&2
  exit 1
}
[ "$(python3 -B "$IR_REFERENCE" run "$INHERITED")" = 70 ] || {
  echo "checked-IR-v4 backend: inherited CKIR3 regression result is not 70" >&2
  exit 1
}

for CASE in canonical inherited; do
  case "$CASE" in
    canonical) INPUT=$CANONICAL ;;
    inherited) INPUT=$INHERITED ;;
  esac
  observe "$CASE-native" 0 30 "$INPUT" "$T/$CASE.native.elf" "$T/backend.native"
  observe "$CASE-self" 0 30 "$INPUT" "$T/$CASE.self.elf" "$T/backend.self"
  cmp "$T/$CASE.native.elf" "$T/$CASE.self.elf" || {
    echo "checked-IR-v4 backend FAIL - $CASE native/self artifact mismatch" >&2
    exit 1
  }
  observe "$CASE-independent-check" 0 30 /dev/null "$T/$CASE.reference.out" \
    python3 -B "$ELF_REFERENCE" check "$INPUT" "$T/$CASE.native.elf"
done

observe canonical-template-check 0 30 /dev/null "$T/templates.out" \
  python3 -B "$FIXTURE" check-artifact "$CANONICAL" "$T/canonical.native.elf"
cmp "$T/inherited.native.elf" "$T/inherited-v3-reference.elf" || {
  echo "checked-IR-v4 backend FAIL - promoted CKIR3 artifact drifted from CKIR3" >&2
  exit 1
}

# Deterministic publication on a repeated native observation.
observe canonical-native-repeat 0 30 "$CANONICAL" "$T/canonical.repeat.elf" \
  "$T/backend.native"
cmp "$T/canonical.native.elf" "$T/canonical.repeat.elf" || {
  echo "checked-IR-v4 backend FAIL - repeated publication differs" >&2
  exit 1
}

CASE_COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  CASE_COUNT=$((CASE_COUNT + 1))
  INPUT="$T/cases/$NAME.ckir4"
  observe "$NAME-native" "$EXPECTED" 30 "$INPUT" "$T/$NAME.native.out" \
    "$T/backend.native"
  assert_empty "$T/$NAME.native.out" "$NAME native"
  observe "$NAME-self" "$EXPECTED" 30 "$INPUT" "$T/$NAME.self.out" \
    "$T/backend.self"
  assert_empty "$T/$NAME.self.out" "$NAME self"
  observe "$NAME-reference" "$EXPECTED" 30 /dev/null "$T/$NAME.reference.out" \
    python3 -B "$IR_REFERENCE" validate "$INPUT"
  assert_empty "$T/$NAME.reference.out" "$NAME reference"
done < "$T/cases/manifest.tsv"
[ "$CASE_COUNT" -eq 15 ] || {
  echo "checked-IR-v4 backend: mutation census $CASE_COUNT, expected 15" >&2
  exit 1
}

# The independent reconstructor checks every artifact byte, truncation, and a
# trailing byte. A constructor-template mutation must also be diagnosed.
observe canonical-artifact-mutation-sweep 0 30 /dev/null "$T/sweep.out" \
  python3 -B "$ELF_REFERENCE" mutation-sweep "$CANONICAL" "$T/canonical.native.elf"
observe mutate-constructor-artifact 0 30 /dev/null "$T/mutate.out" \
  python3 -B "$FIXTURE" mutate-artifact "$T/canonical.native.elf" "$T/mutated.elf"
observe reject-mutated-constructor-artifact 251 30 /dev/null "$T/mismatch.out" \
  python3 -B "$ELF_REFERENCE" check "$CANONICAL" "$T/mutated.elf"
assert_empty "$T/mismatch.out" "constructor artifact reference"

python3 - "$T/timings.tsv" "$PROCEDURES" "$FIELDS" "$LOCALS" \
    "$SELF_ASM_BYTES" "$CASE_COUNT" "$T/templates.out" "$GATE_STARTED" <<'PY'
from pathlib import Path
import sys
import time

rows = []
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    elapsed, label = line.split("\t", 1)
    rows.append((float(elapsed), label))
build_labels = {
    "build-cargo", "build-lowermachine", "build-backend-native",
    "build-backend-self-source", "build-backend-self-link",
    "build-backend-self-sign",
}
build = sum(elapsed for elapsed, label in rows if label in build_labels)
controls = sum(elapsed for elapsed, label in rows if label not in build_labels)
slowest = max(rows, default=(0.0, "none"))
templates = Path(sys.argv[7]).read_text(encoding="ascii").strip()
wall = time.time() - float(sys.argv[8])
print(
    "checked-IR-v4 backend: canonical nested constructors and promoted CKIR3 "
    "regression produce exact native/self/independent ELF bytes; "
    f"{sys.argv[6]} isolated semantic/resource controls include direct-edge=251, "
    "malformed-five=251, valid-five=252; complete artifact mutation sweep passed; "
    f"metadata procedures={sys.argv[2]}/127 fields={sys.argv[3]}/255 "
    f"locals={sys.argv[4]}/32 self-asm={sys.argv[5]}/1048576B; "
    f"observed-build={build:.2f}s observed-controls={controls:.2f}s "
    f"wall={wall:.2f}s "
    f"slowest={slowest[1]}:{slowest[0]:.2f}s; {templates}"
)
PY
