#!/usr/bin/env sh
# Lower-rung meaning probe for exact OMGCOMP -> canonical OMGRSW1 resolution.
# The exhaustive native/self/resource matrix remains in the companion gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "resolution handoff meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "resolution handoff meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolution handoff meaning: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
NEGATIVES="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_negatives.py"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/resolution_handoff_reference.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
for FILE in "$RESOLVER" "$FIXTURE" "$NEGATIVES" "$REFERENCE" "$DECODER"; do
  [ -f "$FILE" ] || { echo "resolution handoff meaning: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "resolution handoff meaning FAIL - Beta compiler artifact" >&2
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
  echo "resolution handoff meaning FAIL - omega2gamma build" >&2
  exit 1
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "resolution handoff meaning FAIL - Gamma interpreter build" >&2
  exit 1
}

python3 - "$T/elaborate.exe" "$RESOLVER" "$T/resolver.gamma" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

elaborator, source_name, output_name = sys.argv[1:]
timeout = 90
ceiling = 1_048_576
started = time.monotonic()
print(f"resolution handoff meaning: START elaboration (timeout {timeout}s)", flush=True)
try:
    with open(source_name, "rb") as source, open(output_name, "wb") as output:
        result = subprocess.run(
            [elaborator], stdin=source, stdout=output, stderr=subprocess.PIPE,
            timeout=timeout, check=False,
        )
except subprocess.TimeoutExpired:
    raise SystemExit(f"resolution handoff meaning FAIL - elaboration exceeded {timeout}s")
elapsed = time.monotonic() - started
payload = Path(output_name).read_bytes()
if result.returncode != 0:
    detail = result.stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"resolution handoff meaning FAIL - elaboration status {result.returncode}: {detail}"
    )
if not payload or b"E2G-UNSUPPORTED" in payload or len(payload) > ceiling:
    raise SystemExit(
        f"resolution handoff meaning FAIL - Gamma bytes {len(payload)} "
        f"outside 1..={ceiling} or unsupported"
    )
print(
    f"resolution handoff meaning: PASS elaboration {len(payload)} bytes "
    f"in {elapsed:.2f}s (ceiling {ceiling})",
    flush=True,
)
PY

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
python3 "$FIXTURE" build "$T/canonical"
python3 "$NEGATIVES" build "$T/negatives"
python3 "$NEGATIVES" check "$T/negatives"
python3 "$REFERENCE" build-controls "$T/controls"

native_case() {
  LABEL=$1
  INPUT=$2
  EXPECTED=$3
  OUTPUT=$4
  set +e
  "$T/resolver.native" < "$INPUT" > "$OUTPUT"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "resolution handoff meaning FAIL - native $LABEL status $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$OUTPUT" ]; then
    echo "resolution handoff meaning FAIL - native $LABEL published rejection bytes" >&2
    exit 1
  fi
}

CANONICAL="$T/canonical/compilation-envelope.bin"
SEMANTIC="$T/negatives/private-import/compilation-envelope.bin"
RESOURCE="$T/controls/identifier-65.omgc"
native_case canonical "$CANONICAL" 0 "$T/canonical.expected"
native_case semantic-251 "$SEMANTIC" 251 "$T/semantic.expected"
native_case resource-252 "$RESOURCE" 252 "$T/resource.expected"
python3 "$REFERENCE" check-canonical "$CANONICAL" "$T/canonical.expected"

run_gamma() { # label input expected expected-output timeout
  python3 - "$1" "$T/interp.exe" "$T/resolver.gamma" "$2" "$3" "$4" "$5" "$T" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

label, interpreter, template_name, input_name, expected, output_name, timeout, temp = sys.argv[1:]
template = Path(template_name).read_text(encoding="ascii")
if template.count("STDIN") != 1:
    raise SystemExit(f"resolution handoff meaning FAIL - {label} placeholder count")
stdin = "Nil"
for byte in reversed(Path(input_name).read_bytes()):
    stdin = f"(Cons {byte} {stdin})"
program = template.replace("STDIN", stdin).encode("ascii")
timeout = float(timeout)
started = time.monotonic()
print(f"resolution handoff meaning: START {label} (timeout {timeout:.0f}s)", flush=True)
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
        raise SystemExit(f"resolution handoff meaning FAIL - {label} exceeded {timeout:.0f}s")
    try:
        stdout, stderr = process.communicate(timeout=min(heartbeat, remaining))
        break
    except subprocess.TimeoutExpired:
        print(
            f"resolution handoff meaning: WAIT {label} "
            f"{time.monotonic()-started:.2f}s of {timeout:.0f}s",
            flush=True,
        )
elapsed = time.monotonic() - started
if process.returncode != 0:
    detail = stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"resolution handoff meaning FAIL - {label} interpreter status "
        f"{process.returncode}: {detail}"
    )
observation = Path(temp) / f"{label}.observation"
observation.write_bytes(stdout)
print(f"resolution handoff meaning: PASS {label} interpreter in {elapsed:.2f}s", flush=True)
PY
  STATUS=$(python3 "$DECODER" "$T/$1.observation" "$T/$1.stdout")
  [ "$STATUS" -eq "$3" ] || {
    echo "resolution handoff meaning FAIL - $1 status $STATUS, expected $3" >&2
    exit 1
  }
  cmp "$T/$1.stdout" "$4" >/dev/null || {
    echo "resolution handoff meaning FAIL - $1 stdout bytes differ" >&2
    exit 1
  }
  echo "resolution handoff meaning: PASS $1 => status $3, exact stdout"
}

run_gamma canonical "$CANONICAL" 0 "$T/canonical.expected" 180
run_gamma semantic-251 "$SEMANTIC" 251 "$T/semantic.expected" 120
run_gamma resource-252 "$RESOURCE" 252 "$T/resource.expected" 120

echo "resolution handoff meaning: canonical 0 / semantic 251 / resource 252 agree through canonical Gamma"
