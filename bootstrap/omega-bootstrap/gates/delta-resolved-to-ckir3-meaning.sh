#!/usr/bin/env sh
# Focused lower-rung meaning probe for OMGLOW3 -> CKIR3.  The companion
# producer gate owns native/self equality and the exhaustive feature/resource
# matrix; this gate owns one representative 0/251/252 observation through the
# persisted Beta -> Gamma route and exact publication agreement with native.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "resolved-to-CKIR3 meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "resolved-to-CKIR3 meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolved-to-CKIR3 meaning: skipped ($TOOL absent)"
    exit 0
  }
done

LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir3.alp"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-frame.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py"
SEMANTICS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v3_reference.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
for FILE in "$LOWERER" "$RESOLVER" "$FRAME" "$FIXTURE" \
  "$SEMANTICS" "$DECODER"; do
  [ -f "$FILE" ] || { echo "resolved-to-CKIR3 meaning: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"

stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "resolved-to-CKIR3 meaning FAIL - Beta compiler artifact" >&2
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
  echo "resolved-to-CKIR3 meaning FAIL - omega2gamma build" >&2
  exit 1
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "resolved-to-CKIR3 meaning FAIL - Gamma interpreter build" >&2
  exit 1
}

# Measured 2026-08-24 baseline: 1,219,982 bytes in about 1.8s. 1,470,464
# leaves a deliberate 20.5% expansion allowance. Reuse this one elaboration
# for every observation so translation cost is not multiplied by the matrix.
python3 - "$T/elaborate.exe" "$LOWERER" "$T/lowerer.gamma" "$T/timings.tsv" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

elaborator, source_name, output_name, timing_name = sys.argv[1:]
timeout = 30
ceiling = 1_470_464
started = time.monotonic()
print(f"resolved-to-CKIR3 meaning: START elaboration (timeout {timeout}s)", flush=True)
try:
    with open(source_name, "rb") as source, open(output_name, "wb") as output:
        result = subprocess.run(
            [elaborator], stdin=source, stdout=output, stderr=subprocess.PIPE,
            timeout=timeout, check=False,
        )
except subprocess.TimeoutExpired:
    raise SystemExit(f"resolved-to-CKIR3 meaning FAIL - elaboration exceeded {timeout}s")
elapsed = time.monotonic() - started
payload = Path(output_name).read_bytes()
if result.returncode != 0:
    detail = result.stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"resolved-to-CKIR3 meaning FAIL - elaboration status {result.returncode}: {detail}"
    )
if not payload or b"E2G-UNSUPPORTED" in payload or len(payload) > ceiling:
    raise SystemExit(
        f"resolved-to-CKIR3 meaning FAIL - Gamma bytes {len(payload)} "
        f"outside 1..={ceiling} or unsupported"
    )
with open(timing_name, "a", encoding="ascii") as timings:
    timings.write(f"{elapsed:.6f}\telaboration\n")
print(
    f"resolved-to-CKIR3 meaning: PASS elaboration {len(payload)} bytes "
    f"in {elapsed:.2f}s (measured ceiling {ceiling})",
    flush=True,
)
PY

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null

# One compact general positive keeps the lower-rung observation focused while
# still requiring the CKIR3 additions: a normalized typed constant DAG,
# opcode 11 CopyAggregateConst, and opcode 12 LessEqual. Guardless transition
# and cyclic interval custody remain in the exhaustive native/self producer
# gate; they are not duplicated into this representative meaning probe.
python3 - "$T/meaning-positive.omg" <<'PY'
from pathlib import Path
import sys

Path(sys.argv[1]).write_text(r'''data MeaningPair [copy] {
    left: u8;
    right: u8;
}

data MeaningProbe {
    pair: MeaningPair;
}

machine MeaningProbe::run(&mut self) -> u8 {
    self.pair = MeaningPair { right: 70, left: 11 };
    transition self.pair.right <= 70 {
        true -> pass()
        _ -> fail()
    }

    state pass(&mut self) { self.pair.right }
    state fail(&mut self) { 71 }
}
''', encoding="ascii")
PY
python3 -B "$FIXTURE" build "$T/canonical.omgc" MeaningProbe run "$T/meaning-positive.omg"
"$T/resolver.native" < "$T/canonical.omgc" > "$T/canonical.omgrsw1"
python3 -B "$FRAME" pack "$T/canonical.omgc" "$T/canonical.omgrsw1" > "$T/canonical.omglow3"

# Preserve valid OMGLOW3 framing while selecting the same phase-local witness
# relations used by the focused native/self gate: a contradictory selected
# root is semantic 251, while a declared witness type table above 2,048 is 252.
python3 - "$T/canonical.omgrsw1" "$T/semantic.omgrsw1" "$T/resource.omgrsw1" <<'PY'
from pathlib import Path
import struct
import sys

canonical = Path(sys.argv[1]).read_bytes()
semantic = bytearray(canonical)
struct.pack_into("<I", semantic, 64, 3)
Path(sys.argv[2]).write_bytes(semantic)
resource = bytearray(canonical)
struct.pack_into("<I", resource, 36, 2049)
Path(sys.argv[3]).write_bytes(resource)
PY
python3 -B "$FRAME" pack "$T/canonical.omgc" "$T/semantic.omgrsw1" > "$T/semantic.omglow3"
python3 -B "$FRAME" pack "$T/canonical.omgc" "$T/resource.omgrsw1" > "$T/resource.omglow3"
: > "$T/empty.expected"

native_case() { # label input expected-status output
  set +e
  "$T/lowerer.native" < "$2" > "$4"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$3" ] || {
    echo "resolved-to-CKIR3 meaning FAIL - native $1 status $ACTUAL, expected $3" >&2
    exit 1
  }
  if [ "$3" -ne 0 ] && [ -s "$4" ]; then
    echo "resolved-to-CKIR3 meaning FAIL - native $1 published rejection bytes" >&2
    exit 1
  fi
}
native_case canonical "$T/canonical.omglow3" 0 "$T/canonical.expected"
native_case semantic-251 "$T/semantic.omglow3" 251 "$T/semantic.expected"
native_case resource-252 "$T/resource.omglow3" 252 "$T/resource.expected"

# Pin the representative positive to the intended CKIR3 feature family before
# asking the independent CKIR3 reference to derive its result.
[ "$(python3 -B "$FIXTURE" inspect "$T/canonical.expected")" = \
  "types=5 constants=3 children=2 ops=14 opcodes=1,2,3,5,11,12 roots=1" ] || {
  echo "resolved-to-CKIR3 meaning FAIL - canonical feature shape changed" >&2
  exit 1
}
[ "$(python3 -B "$SEMANTICS" run "$T/canonical.expected")" = 70 ] || {
  echo "resolved-to-CKIR3 meaning FAIL - canonical CKIR3 result is not 70" >&2
  exit 1
}

run_gamma() { # label input expected-status expected-output timeout
  python3 - "$1" "$T/interp.exe" "$T/lowerer.gamma" "$2" "$3" "$4" "$5" \
    "$T" "$T/timings.tsv" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

label, interpreter, template_name, input_name, expected, output_name, timeout, temp, timing_name = sys.argv[1:]
template = Path(template_name).read_text(encoding="ascii")
if template.count("STDIN") != 1:
    raise SystemExit(f"resolved-to-CKIR3 meaning FAIL - {label} placeholder count")
stdin = "Nil"
for byte in reversed(Path(input_name).read_bytes()):
    stdin = f"(Cons {byte} {stdin})"
program = template.replace("STDIN", stdin).encode("ascii")
timeout = float(timeout)
started = time.monotonic()
print(
    f"resolved-to-CKIR3 meaning: START {label} "
    f"({len(program)} bytes, timeout {timeout:.0f}s)",
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
        raise SystemExit(f"resolved-to-CKIR3 meaning FAIL - {label} exceeded {timeout:.0f}s")
    try:
        stdout, stderr = process.communicate(timeout=min(heartbeat, remaining))
        break
    except subprocess.TimeoutExpired:
        print(
            f"resolved-to-CKIR3 meaning: WAIT {label} "
            f"{time.monotonic()-started:.2f}s of {timeout:.0f}s",
            flush=True,
        )
elapsed = time.monotonic() - started
if process.returncode != 0:
    detail = stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"resolved-to-CKIR3 meaning FAIL - {label} interpreter status "
        f"{process.returncode}: {detail}"
    )
(Path(temp) / f"{label}.observation").write_bytes(stdout)
with open(timing_name, "a", encoding="ascii") as timings:
    timings.write(f"{elapsed:.6f}\t{label}\n")
print(
    f"resolved-to-CKIR3 meaning: PASS {label} interpreter in {elapsed:.2f}s",
    flush=True,
)
PY
  ACTUAL=$(python3 "$DECODER" "$T/$1.observation" "$T/$1.stdout")
  [ "$ACTUAL" -eq "$3" ] || {
    echo "resolved-to-CKIR3 meaning FAIL - $1 status $ACTUAL, expected $3" >&2
    exit 1
  }
  cmp "$T/$1.stdout" "$4" >/dev/null || {
    echo "resolved-to-CKIR3 meaning FAIL - $1 stdout bytes differ" >&2
    exit 1
  }
  echo "resolved-to-CKIR3 meaning: PASS $1 => status $3, exact stdout"
}

run_gamma canonical "$T/canonical.omglow3" 0 "$T/canonical.expected" 120
run_gamma semantic-251 "$T/semantic.omglow3" 251 "$T/empty.expected" 120
run_gamma resource-252 "$T/resource.omglow3" 252 "$T/empty.expected" 120

python3 - "$T/timings.tsv" "$T/canonical.expected" <<'PY'
from pathlib import Path
import sys

rows = []
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    seconds, label = line.split("\t", 1)
    rows.append((label, float(seconds)))
print(
    "resolved-to-CKIR3 meaning timings: "
    + " ".join(f"{label}={seconds:.2f}s" for label, seconds in rows)
    + f" publication={Path(sys.argv[2]).stat().st_size}B"
)
PY
echo "resolved-to-CKIR3 meaning: constant-DAG/CopyAggregateConst/<= CKIR3 result 70 and semantic 251/resource 252 empty-output observations agree exactly through canonical Gamma"
