#!/usr/bin/env sh
# Lower-rung meaning for the CKIR1 artifact tranche. Each Delta compiler program
# is elaborated once through the persisted Beta-written omega2gamma route, then
# canonical Gamma execution must reproduce native status and every published
# byte for one positive, one semantic rejection, and one exhaustion.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "source-custody artifact meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "source-custody artifact meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "source-custody artifact meaning: skipped ($TOOL absent)"
    exit 0
  }
done

PRODUCER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-source-custody-check.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-to-elf.alp"
BUNDLER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_bundle.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
for REQUIRED in "$PRODUCER" "$BACKEND" "$BUNDLER" "$DECODER"; do
  [ -f "$REQUIRED" ] || {
    echo "source-custody artifact meaning: required input absent: $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_ARTIFACT_MEANING_TEMP:-0}" = 1 ]; then
    echo "source-custody artifact meaning: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "source-custody artifact meaning FAIL - Beta compiler artifact" >&2
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
  echo "source-custody artifact meaning FAIL - omega2gamma build" >&2
  exit 1
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "source-custody artifact meaning FAIL - Gamma interpreter build" >&2
  exit 1
}

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER" "$T/producer.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null

# Compiler-sized elaborations are explicit, separately timed resources. Reuse
# each result for all cases instead of paying the same translation repeatedly.
elaborate() { # label source output ceiling
  python3 - "$1" "$T/elaborate.exe" "$2" "$3" "$4" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

label, elaborator, source_name, output_name, ceiling_text = sys.argv[1:]
timeout = 90
started = time.monotonic()
print(f"source-custody artifact meaning: START {label} elaboration (timeout {timeout}s)", flush=True)
try:
    with open(source_name, "rb") as source, open(output_name, "wb") as output:
        result = subprocess.run(
            [elaborator], stdin=source, stdout=output, stderr=subprocess.PIPE,
            timeout=timeout, check=False,
        )
except subprocess.TimeoutExpired:
    raise SystemExit(f"source-custody artifact meaning FAIL - {label} elaboration timeout")
elapsed = time.monotonic() - started
if result.returncode != 0:
    detail = result.stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"source-custody artifact meaning FAIL - {label} elaboration status "
        f"{result.returncode}: {detail}"
    )
payload = Path(output_name).read_bytes()
ceiling = int(ceiling_text)
if not payload or b"E2G-UNSUPPORTED" in payload or len(payload) > ceiling:
    raise SystemExit(
        f"source-custody artifact meaning FAIL - {label} Gamma bytes "
        f"{len(payload)} outside 1..={ceiling} or unsupported"
    )
print(
    f"source-custody artifact meaning: PASS {label} elaboration "
    f"{len(payload)} bytes in {elapsed:.2f}s",
    flush=True,
)
PY
}
elaborate producer "$PRODUCER" "$T/producer.gamma" 2097152
elaborate backend "$BACKEND" "$T/backend.gamma" 2097152

mkdir "$T/sources"
python3 - "$T/sources" <<'PY'
from pathlib import Path
import sys

out = Path(sys.argv[1])
(out / "positive.omg").write_text(r'''
data MeaningLibrary { value: u8; }
''', encoding="ascii")
(out / "entry.omg").write_text(r'''
data MeaningProbe { value: u8; }
machine MeaningProbe::run(&mut self) -> u8 {
    self.value = 70;
    transition self.value < 71 {
        true -> present()
        _ -> absent()
    }
    state present(&mut self) { self.value }
    state absent(&mut self) { 71 }
}
''', encoding="ascii")
(out / "reject.omg").write_text(r'''
data Buffer { bytes: [u8; 8] in Trapping; length: u32 [0..=8]; }
machine Buffer::bad(&self, at: u32 in Trapping) -> u8 { self.bytes[at] }
''', encoding="ascii")
(out / "exhaust.omg").write_text(
    "data ArrayHost { bytes: [u8; 65537] in Trapping; }\n", encoding="ascii"
)
PY
python3 "$BUNDLER" pack "main.omg=$T/sources/positive.omg" > "$T/positive.bundle"
python3 "$BUNDLER" pack "main.omg=$T/sources/entry.omg" > "$T/entry.bundle"
python3 "$BUNDLER" pack "main.omg=$T/sources/reject.omg" > "$T/reject.bundle"
python3 "$BUNDLER" pack "main.omg=$T/sources/exhaust.omg" > "$T/exhaust.bundle"

native_case() { # label executable input expected output
  LABEL=$1
  set +e
  "$2" < "$3" > "$5"
  STATUS=$?
  set -e
  [ "$STATUS" -eq "$4" ] || {
    echo "source-custody artifact meaning FAIL - $LABEL native status $STATUS, expected $4" >&2
    exit 1
  }
}
native_case producer-positive "$T/producer.native" "$T/positive.bundle" 0 "$T/positive.ckir"
native_case producer-entry "$T/producer.native" "$T/entry.bundle" 0 "$T/entry.ckir"
native_case producer-reject "$T/producer.native" "$T/reject.bundle" 251 "$T/reject.ckir"
native_case producer-exhaust "$T/producer.native" "$T/exhaust.bundle" 252 "$T/exhaust.ckir"
[ -s "$T/positive.ckir" ] && [ ! -s "$T/reject.ckir" ] && [ ! -s "$T/exhaust.ckir" ]

python3 - "$T/entry.ckir" "$T/malformed.ckir" "$T/exhausted.ckir" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
malformed = bytearray(source)
malformed[0] ^= 1
Path(sys.argv[2]).write_bytes(malformed)
exhausted = bytearray(source)
struct.pack_into("<I", exhausted, 24, 8_193)
Path(sys.argv[3]).write_bytes(exhausted)
PY
native_case backend-positive "$T/backend.native" "$T/entry.ckir" 0 "$T/positive.elf"
native_case backend-reject "$T/backend.native" "$T/malformed.ckir" 251 "$T/malformed.elf"
native_case backend-exhaust "$T/backend.native" "$T/exhausted.ckir" 252 "$T/exhausted.elf"
[ -s "$T/positive.elf" ] && [ ! -s "$T/malformed.elf" ] && [ ! -s "$T/exhausted.elf" ]

run_gamma() { # label template input expected expected-output timeout
  python3 - "$1" "$T/interp.exe" "$2" "$3" "$4" "$5" "$6" "$T" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

label, interpreter, template_name, input_name, expected_text, output_name, timeout_text, temp = sys.argv[1:]
template = Path(template_name).read_text(encoding="ascii")
if template.count("STDIN") != 1:
    raise SystemExit(f"source-custody artifact meaning FAIL - {label} placeholder count")
stdin = "Nil"
for byte in reversed(Path(input_name).read_bytes()):
    stdin = f"(Cons {byte} {stdin})"
program = template.replace("STDIN", stdin).encode("ascii")
timeout = float(timeout_text)
started = time.monotonic()
print(f"source-custody artifact meaning: START {label} (timeout {timeout:.0f}s)", flush=True)
try:
    result = subprocess.run(
        [interpreter], input=program, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        timeout=timeout, check=False,
    )
except subprocess.TimeoutExpired:
    raise SystemExit(f"source-custody artifact meaning FAIL - {label} timeout")
elapsed = time.monotonic() - started
if result.returncode != 0:
    detail = result.stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"source-custody artifact meaning FAIL - {label} interpreter status "
        f"{result.returncode}: {detail}"
    )
observation = Path(temp) / f"{label}.observation"
observation.write_bytes(result.stdout)
print(f"source-custody artifact meaning: PASS {label} Gamma in {elapsed:.2f}s", flush=True)
PY
  STATUS=$(python3 "$DECODER" "$T/$1.observation" "$T/$1.lower")
  [ "$STATUS" -eq "$4" ] || {
    echo "source-custody artifact meaning FAIL - $1 status $STATUS, expected $4" >&2
    exit 1
  }
  cmp "$T/$1.lower" "$5" || {
    echo "source-custody artifact meaning FAIL - $1 published bytes differ" >&2
    exit 1
  }
}

run_gamma producer-positive "$T/producer.gamma" "$T/positive.bundle" 0 "$T/positive.ckir" 120
run_gamma producer-reject "$T/producer.gamma" "$T/reject.bundle" 251 "$T/reject.ckir" 90
run_gamma producer-exhaust "$T/producer.gamma" "$T/exhaust.bundle" 252 "$T/exhaust.ckir" 90
run_gamma backend-positive "$T/backend.gamma" "$T/entry.ckir" 0 "$T/positive.elf" 180
run_gamma backend-reject "$T/backend.gamma" "$T/malformed.ckir" 251 "$T/malformed.elf" 90
run_gamma backend-exhaust "$T/backend.gamma" "$T/exhausted.ckir" 252 "$T/exhausted.elf" 90

echo "source-custody artifact meaning: producer/backend 0/251/252 status and every published byte agree through canonical Gamma"
