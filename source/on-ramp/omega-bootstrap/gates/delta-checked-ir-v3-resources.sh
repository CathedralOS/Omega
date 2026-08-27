#!/usr/bin/env sh
# Focused canonical CKIR3 resource-boundary gate. Synthetic CKIR is used for
# backend aggregate capacities that cannot be reached through the smaller
# source-unit/statement profile. The independent generator checks canonical
# relations before encoding; native backend and independent references remain
# separate consumers.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "checked-IR-v3 resources: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v3 resources: skipped (compiler construction requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign rg; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v3 resources: skipped ($TOOL absent)"
    exit 0
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v3-to-elf.alp"
GENERATOR="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v3_resources.py"
IR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v3_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v3_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
for REQUIRED in "$BACKEND" "$GENERATOR" "$IR_REFERENCE" "$ELF_REFERENCE" "$LOWERMACHINE"; do
  [ -f "$REQUIRED" ] || {
    echo "checked-IR-v3 resources: required input absent: $REQUIRED" >&2
    exit 1
  }
done
PROCEDURES=$(rg -c '^machine ' "$BACKEND")
[ "$PROCEDURES" -lt 128 ] || {
  echo "checked-IR-v3 resources: $PROCEDURES procedures exceeds Delta envelope" >&2
  exit 1
}

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_CKIR3_RESOURCE_TEMP:-0}" = 1 ]; then
    echo "checked-IR-v3 resources: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT
: > "$T/timings.tsv"

# All compiler-sized commands use one process-group timeout/capture path. An
# expected nonzero backend/reference status is a successful gate observation.
python3 - "$T/observe.py" <<'PY'
from pathlib import Path
import sys

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
try:
    with open(input_name, "rb") as source:
        process = subprocess.Popen(
            command,
            stdin=source,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        stdout, stderr = process.communicate(timeout=timeout)
except subprocess.TimeoutExpired:
    elapsed = time.monotonic() - started
    os.killpg(process.pid, signal.SIGKILL)
    stdout, stderr = process.communicate()
    Path(output_name).write_bytes(stdout)
    Path(output_name + ".stderr").write_bytes(stderr)
    print(f"checked-IR-v3 resources FAIL - {label} exceeded {timeout:.0f}s", file=sys.stderr)
    raise SystemExit(1)

elapsed = time.monotonic() - started
Path(output_name).write_bytes(stdout)
Path(output_name + ".stderr").write_bytes(stderr)
with open(timing_name, "a", encoding="utf-8") as timings:
    timings.write(f"{elapsed:.6f}\t{label}\n")
if process.returncode != expected:
    print(
        f"checked-IR-v3 resources FAIL - {label} status {process.returncode}, "
        f"expected {expected} ({elapsed:.2f}s)",
        file=sys.stderr,
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
    echo "checked-IR-v3 resources FAIL - $2 published bytes" >&2
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

mkdir "$T/cases"
observe generate-resources 0 30 /dev/null "$T/generator.stdout" \
  python3 -B "$GENERATOR" "$T/cases"

TAB=$(printf '\t')
CASE_COUNT=0
SELF_COUNT=0
SUCCESS_COUNT=0
ELF_COUNT=0
REFERENCE_ACCEPT_COUNT=0
REFERENCE_REJECT_COUNT=0
while IFS="$TAB" read -r NAME EXPECTED_STATUS REFERENCE_VALID EXPECTED_OUTPUT SELF_REPRESENTATIVE ENCODED_BYTES NOTE; do
  [ "$NAME" != name ] || continue
  CASE_COUNT=$((CASE_COUNT + 1))
  CASE=${NAME%.ckir3}
  INPUT="$T/cases/$NAME"
  [ "$(wc -c < "$INPUT" | tr -d ' ')" -eq "$ENCODED_BYTES" ] || {
    echo "checked-IR-v3 resources FAIL - $NAME encoded length drift" >&2
    exit 1
  }
  case "$REFERENCE_VALID" in
    true) REFERENCE_STATUS=0; REFERENCE_ACCEPT_COUNT=$((REFERENCE_ACCEPT_COUNT + 1)) ;;
    false) REFERENCE_STATUS=1; REFERENCE_REJECT_COUNT=$((REFERENCE_REJECT_COUNT + 1)) ;;
    *) echo "checked-IR-v3 resources FAIL - bad reference policy: $NAME" >&2; exit 1 ;;
  esac
  observe "reference-$CASE" "$REFERENCE_STATUS" 60 /dev/null \
    "$T/$CASE.reference.stdout" python3 -B "$IR_REFERENCE" validate "$INPUT"

  observe "native-$CASE" "$EXPECTED_STATUS" 90 "$INPUT" \
    "$T/$CASE.native.output" "$T/backend.native"
  if [ "$EXPECTED_STATUS" -ne 0 ]; then
    assert_empty "$T/$CASE.native.output" "$CASE native rejection"
  else
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    case "$EXPECTED_OUTPUT" in
      empty)
        assert_empty "$T/$CASE.native.output" "$CASE native library"
        ;;
      elf|elf:*)
        ELF_COUNT=$((ELF_COUNT + 1))
        [ -s "$T/$CASE.native.output" ] || {
          echo "checked-IR-v3 resources FAIL - $CASE native emitted no ELF" >&2
          exit 1
        }
        observe "elf-$CASE" 0 90 /dev/null "$T/$CASE.elf-reference.stdout" \
          python3 -B "$ELF_REFERENCE" check "$INPUT" "$T/$CASE.native.output"
        case "$EXPECTED_OUTPUT" in
          elf:*)
            EXPECTED_ELF_BYTES=${EXPECTED_OUTPUT#elf:}
            [ "$(wc -c < "$T/$CASE.native.output" | tr -d ' ')" -eq "$EXPECTED_ELF_BYTES" ] || {
              echo "checked-IR-v3 resources FAIL - $CASE ELF length drift" >&2
              exit 1
            }
            ;;
        esac
        ;;
      *) echo "checked-IR-v3 resources FAIL - bad output policy: $NAME" >&2; exit 1 ;;
    esac
  fi

  case "$SELF_REPRESENTATIVE" in
    true)
      SELF_COUNT=$((SELF_COUNT + 1))
      observe "self-$CASE" "$EXPECTED_STATUS" 120 "$INPUT" \
        "$T/$CASE.self.output" "$T/backend.self"
      if [ "$EXPECTED_STATUS" -ne 0 ] || [ "$EXPECTED_OUTPUT" = empty ]; then
        assert_empty "$T/$CASE.self.output" "$CASE self observation"
      else
        cmp "$T/$CASE.native.output" "$T/$CASE.self.output"
      fi
      ;;
    false) ;;
    *) echo "checked-IR-v3 resources FAIL - bad self policy: $NAME" >&2; exit 1 ;;
  esac
done < "$T/cases/manifest.tsv"

[ "$CASE_COUNT" -eq 14 ] && [ "$SELF_COUNT" -eq 6 ] && \
  [ "$SUCCESS_COUNT" -eq 8 ] && [ "$ELF_COUNT" -eq 7 ] && \
  [ "$REFERENCE_ACCEPT_COUNT" -eq 10 ] && [ "$REFERENCE_REJECT_COUNT" -eq 4 ] || {
  echo "checked-IR-v3 resources FAIL - manifest coverage census drift" >&2
  exit 1
}

python3 - "$T/timings.tsv" <<'PY'
from collections import defaultdict
from pathlib import Path
import sys

rows = []
phases = defaultdict(float)
for line in Path(sys.argv[1]).read_text(encoding="utf-8").splitlines():
    elapsed, label = line.split("\t", 1)
    seconds = float(elapsed)
    rows.append((seconds, label))
    phase = label.split("-", 1)[0]
    phases[phase] += seconds
slowest = max(rows)
phase_text = " ".join(f"{name}={phases[name]:.2f}s" for name in sorted(phases))
print(
    f"checked-IR-v3 resources timings: {phase_text}; total-command-time="
    f"{sum(seconds for seconds, _ in rows):.2f}s; slowest={slowest[1]} {slowest[0]:.2f}s"
)
PY
echo "checked-IR-v3 resources: $CASE_COUNT exhaustive native cases, $SELF_COUNT persisted-self representatives, $REFERENCE_ACCEPT_COUNT/$REFERENCE_REJECT_COUNT independent reference accept/reject, $ELF_COUNT exact ELF reconstructions, and all 0/252 outputs passed"
