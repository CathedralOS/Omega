#!/usr/bin/env sh
# Lower-rung meaning probe for the CKIR2 -> Linux x86-64 ELF backend.  The
# backend is elaborated once through the persisted Beta-written omega2gamma
# route, then that one Gamma program must reproduce native publication and
# status for an accepted call DAG, a semantic rejection, and an exhaustion.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "checked-IR-v2 backend meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v2 backend meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v2 backend meaning: skipped ($TOOL absent)"
    exit 0
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v2-to-elf.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/ckir2_call_reference.py"
SEMANTICS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v2_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v2_reference.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
for REQUIRED in "$BACKEND" "$FIXTURE" "$SEMANTICS" "$ELF_REFERENCE" "$DECODER"; do
  [ -f "$REQUIRED" ] || {
    echo "checked-IR-v2 backend meaning: missing $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
cleanup() {
  if [ "${OMEGA_KEEP_CKIR2_BACKEND_MEANING_TEMP:-0}" = 1 ]; then
    echo "checked-IR-v2 backend meaning: retained $T" >&2
  else
    rm -rf "$T"
  fi
}
trap cleanup EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "checked-IR-v2 backend meaning FAIL - Beta compiler artifact" >&2
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
  echo "checked-IR-v2 backend meaning FAIL - omega2gamma build" >&2
  exit 1
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "checked-IR-v2 backend meaning FAIL - Gamma interpreter build" >&2
  exit 1
}

# Measured 2026-08-24 baseline: 341,738 bytes in 1.08s. 409,600 leaves a
# deliberate 19.9% expansion allowance.  All observations below reuse this
# one elaboration instead of normalizing repeated translation cost.
python3 - "$T/elaborate.exe" "$BACKEND" "$T/backend.gamma" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

elaborator, source_name, output_name = sys.argv[1:]
timeout = 15
ceiling = 409_600
started = time.monotonic()
print(
    f"checked-IR-v2 backend meaning: START elaboration (timeout {timeout}s)",
    flush=True,
)
try:
    with open(source_name, "rb") as source, open(output_name, "wb") as output:
        result = subprocess.run(
            [elaborator], stdin=source, stdout=output, stderr=subprocess.PIPE,
            timeout=timeout, check=False,
        )
except subprocess.TimeoutExpired:
    raise SystemExit(
        f"checked-IR-v2 backend meaning FAIL - elaboration exceeded {timeout}s"
    )
elapsed = time.monotonic() - started
payload = Path(output_name).read_bytes()
if result.returncode != 0:
    detail = result.stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"checked-IR-v2 backend meaning FAIL - elaboration status "
        f"{result.returncode}: {detail}"
    )
if not payload or b"E2G-UNSUPPORTED" in payload or len(payload) > ceiling:
    raise SystemExit(
        f"checked-IR-v2 backend meaning FAIL - Gamma bytes {len(payload)} "
        f"outside 1..={ceiling} or unsupported"
    )
print(
    f"checked-IR-v2 backend meaning: PASS elaboration {len(payload)} bytes "
    f"in {elapsed:.2f}s (measured ceiling {ceiling})",
    flush=True,
)
PY

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null

# The semantic and resource witnesses are byte-local mutations already pinned
# by the exhaustive native/self backend gate: schema-major confusion is 251,
# while a declared operation table beyond 32768 rows is 252.  Both inputs stay
# small enough that Gamma cost measures the backend rather than fixture volume.
python3 "$FIXTURE" emit "$T/canonical.ckir2"
python3 - "$T/canonical.ckir2" "$T/semantic.ckir2" "$T/resource.ckir2" <<'PY'
from pathlib import Path
import struct
import sys

canonical = Path(sys.argv[1]).read_bytes()
semantic = bytearray(canonical)
struct.pack_into("<H", semantic, 8, 1)
Path(sys.argv[2]).write_bytes(semantic)
resource = bytearray(canonical)
struct.pack_into("<I", resource, 24 + 7 * 4, 32_769)
Path(sys.argv[3]).write_bytes(resource)
PY

[ "$(python3 "$SEMANTICS" run "$T/canonical.ckir2")" = 70 ] || {
  echo "checked-IR-v2 backend meaning FAIL - canonical CKIR result is not 70" >&2
  exit 1
}

native_case() { # label input expected output
  LABEL=$1
  set +e
  "$T/backend.native" < "$2" > "$4"
  STATUS=$?
  set -e
  [ "$STATUS" -eq "$3" ] || {
    echo "checked-IR-v2 backend meaning FAIL - $LABEL native status $STATUS, expected $3" >&2
    exit 1
  }
  if [ "$3" -ne 0 ] && [ -s "$4" ]; then
    echo "checked-IR-v2 backend meaning FAIL - $LABEL native published rejection bytes" >&2
    exit 1
  fi
}

native_case canonical "$T/canonical.ckir2" 0 "$T/canonical.expected"
native_case semantic-251 "$T/semantic.ckir2" 251 "$T/semantic.expected"
native_case resource-252 "$T/resource.ckir2" 252 "$T/resource.expected"
python3 "$ELF_REFERENCE" check "$T/canonical.ckir2" "$T/canonical.expected" >/dev/null
[ "$(wc -c < "$T/canonical.expected" | tr -d ' ')" -eq 8192 ] || {
  echo "checked-IR-v2 backend meaning FAIL - canonical ELF is not 8192 bytes" >&2
  exit 1
}

run_gamma() { # label input expected expected-output timeout
  python3 - "$1" "$T/interp.exe" "$T/backend.gamma" "$2" "$3" "$4" "$5" "$T" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

label, interpreter, template_name, input_name, expected, output_name, timeout, temp = sys.argv[1:]
template = Path(template_name).read_text(encoding="ascii")
if template.count("STDIN") != 1:
    raise SystemExit(
        f"checked-IR-v2 backend meaning FAIL - {label} placeholder count"
    )
stdin = "Nil"
for byte in reversed(Path(input_name).read_bytes()):
    stdin = f"(Cons {byte} {stdin})"
program = template.replace("STDIN", stdin).encode("ascii")
timeout = float(timeout)
started = time.monotonic()
print(
    f"checked-IR-v2 backend meaning: START {label} (timeout {timeout:.0f}s)",
    flush=True,
)
process = subprocess.Popen(
    [interpreter], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
)
assert process.stdin is not None
process.stdin.write(program)
process.stdin.close()
process.stdin = None
heartbeat = 15.0
while True:
    remaining = timeout - (time.monotonic() - started)
    if remaining <= 0:
        process.kill()
        process.communicate()
        raise SystemExit(
            f"checked-IR-v2 backend meaning FAIL - {label} exceeded {timeout:.0f}s"
        )
    try:
        stdout, stderr = process.communicate(timeout=min(heartbeat, remaining))
        break
    except subprocess.TimeoutExpired:
        print(
            f"checked-IR-v2 backend meaning: WAIT {label} "
            f"{time.monotonic()-started:.2f}s of {timeout:.0f}s",
            flush=True,
        )
elapsed = time.monotonic() - started
if process.returncode != 0:
    detail = stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"checked-IR-v2 backend meaning FAIL - {label} interpreter status "
        f"{process.returncode}: {detail}"
    )
(Path(temp) / f"{label}.observation").write_bytes(stdout)
print(
    f"checked-IR-v2 backend meaning: PASS {label} interpreter in {elapsed:.2f}s",
    flush=True,
)
PY
  STATUS=$(python3 "$DECODER" "$T/$1.observation" "$T/$1.stdout")
  [ "$STATUS" -eq "$3" ] || {
    echo "checked-IR-v2 backend meaning FAIL - $1 status $STATUS, expected $3" >&2
    exit 1
  }
  cmp "$T/$1.stdout" "$4" >/dev/null || {
    echo "checked-IR-v2 backend meaning FAIL - $1 published bytes differ" >&2
    exit 1
  }
  echo "checked-IR-v2 backend meaning: PASS $1 => status $3, exact stdout"
}

run_gamma canonical "$T/canonical.ckir2" 0 "$T/canonical.expected" 240
run_gamma semantic-251 "$T/semantic.ckir2" 251 "$T/semantic.expected" 180
run_gamma resource-252 "$T/resource.ckir2" 252 "$T/resource.expected" 180
python3 "$ELF_REFERENCE" check "$T/canonical.ckir2" "$T/canonical.stdout" >/dev/null

echo "checked-IR-v2 backend meaning: canonical exact 8192-byte ELF/result 70 and representative 0/251/252 observations agree through canonical Gamma"
