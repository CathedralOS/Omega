#!/usr/bin/env sh
# Lower-rung meaning probe for exact OMGLOW2 -> CKIR2 explicit-root/call lowering.
# The exhaustive native/self relation matrix remains in the companion gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "resolved-to-CKIR2 meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "resolved-to-CKIR2 meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolved-to-CKIR2 meaning: skipped ($TOOL absent)"
    exit 0
  }
done

LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir2.alp"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow2.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/role3_resolution_fixture.py"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/ckir2_call_reference.py"
SEMANTICS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v2_reference.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
for FILE in "$LOWERER" "$RESOLVER" "$FRAME" "$FIXTURE" "$REFERENCE" \
  "$SEMANTICS" "$DECODER"; do
  [ -f "$FILE" ] || { echo "resolved-to-CKIR2 meaning: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "resolved-to-CKIR2 meaning FAIL - Beta compiler artifact" >&2
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
  echo "resolved-to-CKIR2 meaning FAIL - omega2gamma build" >&2
  exit 1
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "resolved-to-CKIR2 meaning FAIL - Gamma interpreter build" >&2
  exit 1
}

# Measured 2026-08-24 baseline: 515,889 bytes. 622,592 is a deliberate 20.7%
# expansion allowance, not the generic 1 MiB ceiling.
python3 - "$T/elaborate.exe" "$LOWERER" "$T/lowerer.gamma" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

elaborator, source_name, output_name = sys.argv[1:]
timeout = 20
ceiling = 622_592
started = time.monotonic()
print(f"resolved-to-CKIR2 meaning: START elaboration (timeout {timeout}s)", flush=True)
try:
    with open(source_name, "rb") as source, open(output_name, "wb") as output:
        result = subprocess.run(
            [elaborator], stdin=source, stdout=output, stderr=subprocess.PIPE,
            timeout=timeout, check=False,
        )
except subprocess.TimeoutExpired:
    raise SystemExit(f"resolved-to-CKIR2 meaning FAIL - elaboration exceeded {timeout}s")
elapsed = time.monotonic() - started
payload = Path(output_name).read_bytes()
if result.returncode != 0:
    detail = result.stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"resolved-to-CKIR2 meaning FAIL - elaboration status {result.returncode}: {detail}"
    )
if not payload or b"E2G-UNSUPPORTED" in payload or len(payload) > ceiling:
    raise SystemExit(
        f"resolved-to-CKIR2 meaning FAIL - Gamma bytes {len(payload)} "
        f"outside 1..={ceiling} or unsupported"
    )
print(
    f"resolved-to-CKIR2 meaning: PASS elaboration {len(payload)} bytes "
    f"in {elapsed:.2f}s (measured ceiling {ceiling})",
    flush=True,
)
PY

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null
python3 "$FIXTURE" build "$T/fixture"
"$T/resolver.native" < "$T/fixture/valid.omgc" > "$T/canonical.omgrsw1"
python3 "$FIXTURE" check "$T/fixture/valid.omgc" "$T/canonical.omgrsw1" >/dev/null
python3 "$FRAME" pack "$T/fixture/valid.omgc" "$T/canonical.omgrsw1" > "$T/canonical.omglow2"
python3 "$REFERENCE" emit "$T/canonical.expected"
[ "$(python3 "$REFERENCE" check "$T/canonical.expected")" = 70 ]
[ "$(python3 "$SEMANTICS" run "$T/canonical.expected")" = 70 ]

# Derive the same representative semantic and resource failures pinned by the
# focused native/self gate.  Framing stays valid; only witness meaning changes.
python3 - "$T/canonical.omgrsw1" "$T/semantic.omgrsw1" "$T/resource.omgrsw1" <<'PY'
import struct
import sys

raw = bytearray(open(sys.argv[1], "rb").read())
semantic = bytearray(raw)
struct.pack_into("<I", semantic, 64, 3)  # selected decoy contradicts OMGCOMP root
open(sys.argv[2], "wb").write(semantic)
resource = bytearray(raw)
struct.pack_into("<I", resource, 36, 2049)  # witness type count exceeds the contract
open(sys.argv[3], "wb").write(resource)
PY
python3 "$FRAME" pack "$T/fixture/valid.omgc" "$T/semantic.omgrsw1" > "$T/semantic.omglow2"
python3 "$FRAME" pack "$T/fixture/valid.omgc" "$T/resource.omgrsw1" > "$T/resource.omglow2"
: > "$T/empty.expected"

native_case() { # label input expected output
  set +e
  "$T/lowerer.native" < "$2" > "$4"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$3" ] || {
    echo "resolved-to-CKIR2 meaning FAIL - native $1 status $ACTUAL, expected $3" >&2
    exit 1
  }
  if [ "$3" -ne 0 ] && [ -s "$4" ]; then
    echo "resolved-to-CKIR2 meaning FAIL - native $1 published rejection bytes" >&2
    exit 1
  fi
}
native_case canonical "$T/canonical.omglow2" 0 "$T/canonical.native"
native_case semantic-251 "$T/semantic.omglow2" 251 "$T/semantic.native"
native_case resource-252 "$T/resource.omglow2" 252 "$T/resource.native"
cmp "$T/canonical.native" "$T/canonical.expected"

run_gamma() { # label input expected expected-output timeout
  python3 - "$1" "$T/interp.exe" "$T/lowerer.gamma" "$2" "$3" "$4" "$5" "$T" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

label, interpreter, template_name, input_name, expected, output_name, timeout, temp = sys.argv[1:]
template = Path(template_name).read_text(encoding="ascii")
if template.count("STDIN") != 1:
    raise SystemExit(f"resolved-to-CKIR2 meaning FAIL - {label} placeholder count")
stdin = "Nil"
for byte in reversed(Path(input_name).read_bytes()):
    stdin = f"(Cons {byte} {stdin})"
program = template.replace("STDIN", stdin).encode("ascii")
timeout = float(timeout)
started = time.monotonic()
print(f"resolved-to-CKIR2 meaning: START {label} (timeout {timeout:.0f}s)", flush=True)
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
        process.kill(); process.communicate()
        raise SystemExit(f"resolved-to-CKIR2 meaning FAIL - {label} exceeded {timeout:.0f}s")
    try:
        stdout, stderr = process.communicate(timeout=min(heartbeat, remaining))
        break
    except subprocess.TimeoutExpired:
        print(
            f"resolved-to-CKIR2 meaning: WAIT {label} "
            f"{time.monotonic()-started:.2f}s of {timeout:.0f}s",
            flush=True,
        )
elapsed = time.monotonic() - started
if process.returncode != 0:
    detail = stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"resolved-to-CKIR2 meaning FAIL - {label} interpreter status "
        f"{process.returncode}: {detail}"
    )
(Path(temp) / f"{label}.observation").write_bytes(stdout)
print(f"resolved-to-CKIR2 meaning: PASS {label} interpreter in {elapsed:.2f}s", flush=True)
PY
  STATUS=$(python3 "$DECODER" "$T/$1.observation" "$T/$1.stdout")
  [ "$STATUS" -eq "$3" ] || {
    echo "resolved-to-CKIR2 meaning FAIL - $1 status $STATUS, expected $3" >&2
    exit 1
  }
  cmp "$T/$1.stdout" "$4" >/dev/null || {
    echo "resolved-to-CKIR2 meaning FAIL - $1 stdout bytes differ" >&2
    exit 1
  }
  echo "resolved-to-CKIR2 meaning: PASS $1 => status $3, exact stdout"
}

run_gamma canonical "$T/canonical.omglow2" 0 "$T/canonical.expected" 300
run_gamma semantic-251 "$T/semantic.omglow2" 251 "$T/empty.expected" 240
run_gamma resource-252 "$T/resource.omglow2" 252 "$T/empty.expected" 240

echo "resolved-to-CKIR2 meaning: canonical exact CKIR2/result 70 and semantic 251/resource 252 empty-output observations agree through canonical Gamma"
