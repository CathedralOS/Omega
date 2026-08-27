#!/usr/bin/env sh
# Focused CKIR5 conservative backend gate. Handcrafted CKIR keeps this lane
# independent of the concurrently evolving resolved-source lowerer.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v5 backend: skipped (compiler construction requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v5 backend: skipped ($TOOL absent)"
    exit 0
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp"
V4_BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v4-to-elf.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-checked-ir-v5-backend-fixture.py"
V4_FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-checked-ir-v4-fixture.py"
IR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v5_reference.py"
V4_ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v4_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for REQUIRED in "$BACKEND" "$V4_BACKEND" "$FIXTURE" "$V4_FIXTURE" \
    "$IR_REFERENCE" "$V4_ELF_REFERENCE" "$LOWERMACHINE"; do
  [ -f "$REQUIRED" ] || { echo "checked-IR-v5 backend: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
STARTED=$(python3 -c 'import time; print(time.time())')
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"

python3 - "$T/observe.py" <<'PY'
from pathlib import Path
import os, signal, subprocess, sys, time
Path(sys.argv[1]).write_text(r'''#!/usr/bin/env python3
from pathlib import Path
import os, signal, subprocess, sys, time
label, expected, timeout, stdin, stdout, timings, *command = sys.argv[1:]
started = time.monotonic()
with open(stdin, "rb") as source:
    process = subprocess.Popen(command, stdin=source, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        out, err = process.communicate(timeout=float(timeout))
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        out, err = process.communicate()
        Path(stdout).write_bytes(out); Path(stdout + ".stderr").write_bytes(err)
        raise SystemExit(f"checked-IR-v5 backend FAIL - {label} timeout")
elapsed = time.monotonic() - started
Path(stdout).write_bytes(out); Path(stdout + ".stderr").write_bytes(err)
with open(timings, "a", encoding="ascii") as log:
    log.write(f"{elapsed:.6f}\t{label}\n")
if process.returncode != int(expected):
    if err: sys.stderr.buffer.write(err[-4096:])
    raise SystemExit(f"checked-IR-v5 backend FAIL - {label} status "
                     f"{process.returncode}, expected {expected}")
''', encoding="utf-8")
PY

observe() { # label status timeout stdin stdout command...
  LABEL=$1; EXPECTED=$2; TIMEOUT=$3; INPUT=$4; OUTPUT=$5; shift 5
  python3 "$T/observe.py" "$LABEL" "$EXPECTED" "$TIMEOUT" "$INPUT" \
    "$OUTPUT" "$T/timings.tsv" "$@"
}
empty() { [ ! -s "$1" ] || { echo "checked-IR-v5 backend FAIL - $2 published bytes" >&2; exit 1; }; }

python3 - "$BACKEND" "$T/metadata.tsv" <<'PY'
from pathlib import Path
import re, sys
source = Path(sys.argv[1]).read_text(encoding="utf-8")
start = source.index("data Main {") + len("data Main {")
depth, end = 1, start
while depth:
    depth += (source[end] == "{") - (source[end] == "}"); end += 1
body = re.sub(r"//.*", "", source[start:end - 1])
fields = sum(sum(bool(x.strip()) for x in s.rsplit("\n", 1)[-1].split(":", 1)[0].split(","))
             for s in body.split(";") if ":" in s)
procedures = len(re.findall(r"^machine ", source, re.MULTILINE))
locals_ = len(re.findall(r"\blet\s+[A-Za-z_]\w*\s*:", source))
locals_ += sum(m.group(1).count(":") for m in re.finditer(r"\bstate\s+\w+\s*\(([^)]*)\)", source))
Path(sys.argv[2]).write_text(f"{procedures}\t{fields}\t{locals_}\n", encoding="ascii")
PY
TAB=$(printf '\t')
IFS="$TAB" read -r PROCEDURES FIELDS LOCALS < "$T/metadata.tsv"
[ "$PROCEDURES" -lt 128 ] && [ "$FIELDS" -le 255 ] && [ "$LOCALS" -le 32 ] || {
  echo "checked-IR-v5 backend: Delta metadata ceiling exceeded" >&2; exit 1;
}

observe build-cargo 0 120 /dev/null "$T/cargo.out" \
  cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
observe build-lowermachine 0 90 /dev/null "$T/lowermachine.out" \
  env DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine"
observe build-v5-native 0 90 /dev/null "$T/v5-native.out" \
  env DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native"
observe build-v4-native 0 90 /dev/null "$T/v4-native.out" \
  env DELTA_ARCH=aarch64 "$DELTA" "$V4_BACKEND" "$T/backend.v4"
observe build-v5-self-source 0 120 "$BACKEND" "$T/backend.self.s" "$T/lowermachine"
observe build-v5-self-link 0 90 /dev/null "$T/link.out" \
  clang -arch arm64 -o "$T/backend.self" "$T/backend.self.s"
observe build-v5-self-sign 0 30 /dev/null "$T/sign.out" \
  codesign -f -s - "$T/backend.self"
SELF_ASM=$(wc -c < "$T/backend.self.s" | tr -d ' ')
[ "$SELF_ASM" -le 1310720 ] || {
  echo "checked-IR-v5 backend: self asm $SELF_ASM exceeds 1310720" >&2; exit 1;
}

observe generate-v5 0 30 /dev/null "$T/generate-v5.out" \
  python3 -B "$FIXTURE" emit "$T/cases"
observe generate-v4 0 30 /dev/null "$T/generate-v4.out" \
  python3 -B "$V4_FIXTURE" emit "$T/v4-cases"
CANONICAL="$T/cases/canonical.ckir5"
observe canonical-reference 0 30 /dev/null "$T/canonical.reference" \
  python3 -B "$FIXTURE" check-ir "$CANONICAL"
observe canonical-native 0 30 "$CANONICAL" "$T/canonical.native.elf" "$T/backend.native"
observe canonical-self 0 30 "$CANONICAL" "$T/canonical.self.elf" "$T/backend.self"
cmp "$T/canonical.native.elf" "$T/canonical.self.elf" || {
  echo "checked-IR-v5 backend: native/self CKIR5 artifact mismatch" >&2; exit 1;
}
observe canonical-template 0 30 /dev/null "$T/template.out" \
  python3 -B "$FIXTURE" check-artifact "$T/canonical.native.elf"

# The successor's CKIR4 branch must be byte-identical to the frozen backend and
# to the existing independent CKIR4 artifact reconstructor.
V4_CANONICAL="$T/v4-cases/canonical.ckir4"
observe v4-frozen 0 30 "$V4_CANONICAL" "$T/v4.frozen.elf" "$T/backend.v4"
observe v4-successor-native 0 30 "$V4_CANONICAL" "$T/v4.successor.elf" "$T/backend.native"
observe v4-successor-self 0 30 "$V4_CANONICAL" "$T/v4.successor-self.elf" "$T/backend.self"
cmp "$T/v4.frozen.elf" "$T/v4.successor.elf"
cmp "$T/v4.frozen.elf" "$T/v4.successor-self.elf"
observe v4-independent 0 30 /dev/null "$T/v4.reference" \
  python3 -B "$V4_ELF_REFERENCE" check "$V4_CANONICAL" "$T/v4.successor.elf"

CASE_COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  CASE_COUNT=$((CASE_COUNT + 1))
  INPUT="$T/cases/$NAME.ckir5"
  observe "$NAME-native" "$EXPECTED" 30 "$INPUT" "$T/$NAME.native" "$T/backend.native"
  observe "$NAME-self" "$EXPECTED" 30 "$INPUT" "$T/$NAME.self" "$T/backend.self"
  empty "$T/$NAME.native" "$NAME native"
  empty "$T/$NAME.self" "$NAME self"
  observe "$NAME-reference" "$EXPECTED" 30 /dev/null "$T/$NAME.reference" \
    python3 -B "$IR_REFERENCE" validate "$INPUT"
  [ "$EXPECTED" -eq 0 ] || empty "$T/$NAME.reference" "$NAME reference"
done < "$T/cases/manifest.tsv"
[ "$CASE_COUNT" -eq 11 ] || { echo "checked-IR-v5 backend: mutation census $CASE_COUNT" >&2; exit 1; }

python3 - "$T/timings.tsv" "$PROCEDURES" "$FIELDS" "$LOCALS" "$SELF_ASM" \
    "$CASE_COUNT" "$STARTED" <<'PY'
from pathlib import Path
import sys, time
rows = [(float(line.split("\t", 1)[0]), line.split("\t", 1)[1])
        for line in Path(sys.argv[1]).read_text().splitlines()]
slowest = max(rows, default=(0.0, "none"))
print("checked-IR-v5 backend: ConstructCase, structural Copy/Call, value/place "
      "CaseDispatch, selected complete payload snapshots, runtime tag checks, "
      "and exact result 70 passed; CKIR4 frozen/successor/self/reference ELF bytes "
      f"match; controls={sys.argv[6]} metadata={sys.argv[2]}/127 procedures "
      f"{sys.argv[3]}/255 fields {sys.argv[4]}/32 locals self-asm={sys.argv[5]}B "
      f"observed={sum(x for x, _ in rows):.2f}s wall={time.time()-float(sys.argv[7]):.2f}s "
      f"slowest={slowest[1]}:{slowest[0]:.2f}s")
PY
