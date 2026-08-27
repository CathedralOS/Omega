#!/usr/bin/env sh
# Positive lower-rung composition for the actual two-package lowerer CKIR and
# the limited backend ELF. Exhaustive and rejection evidence remains in the
# native/self composite and the two component meaning gates.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "two-package composite meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "two-package composite meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "two-package composite meaning: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-to-elf.alp"
FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
CKIR_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_reference.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
for REQUIRED in "$RESOLVER" "$LOWERER" "$BACKEND" "$FRAME" "$FIXTURE" \
  "$CKIR_REFERENCE" "$ELF_REFERENCE" "$DECODER"; do
  [ -f "$REQUIRED" ] || {
    echo "two-package composite meaning: missing $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "two-package composite meaning FAIL - Beta compiler artifact" >&2
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
  echo "two-package composite meaning FAIL - omega2gamma build" >&2
  exit 1
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "two-package composite meaning FAIL - Gamma interpreter build" >&2
  exit 1
}

# Both ceilings are measured allowances, not the older generic 2 MiB bound.
# 2026-08-24 baselines: lowerer 330,440 bytes/1.73s; backend
# 184,758 bytes/0.71s. The ceilings leave 19.0% and 19.7% respectively.
elaborate() { # label source output timeout ceiling
  python3 - "$1" "$T/elaborate.exe" "$2" "$3" "$4" "$5" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

label, elaborator, source_name, output_name, timeout_text, ceiling_text = sys.argv[1:]
timeout = float(timeout_text)
ceiling = int(ceiling_text)
started = time.monotonic()
print(
    f"two-package composite meaning: START {label} elaboration "
    f"(timeout {timeout:.0f}s)",
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
        f"two-package composite meaning FAIL - {label} elaboration exceeded "
        f"{timeout:.0f}s"
    )
elapsed = time.monotonic() - started
payload = Path(output_name).read_bytes()
if result.returncode != 0:
    detail = result.stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"two-package composite meaning FAIL - {label} elaboration status "
        f"{result.returncode}: {detail}"
    )
if not payload or b"E2G-UNSUPPORTED" in payload or len(payload) > ceiling:
    raise SystemExit(
        f"two-package composite meaning FAIL - {label} Gamma bytes "
        f"{len(payload)} outside 1..={ceiling} or unsupported"
    )
print(
    f"two-package composite meaning: PASS {label} elaboration "
    f"{len(payload)} bytes in {elapsed:.2f}s (ceiling {ceiling})",
    flush=True,
)
PY
}
elaborate lowerer "$LOWERER" "$T/lowerer.gamma" 15 393216
elaborate backend "$BACKEND" "$T/backend.gamma" 15 221184

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null

python3 "$FIXTURE" build "$T/canonical"
"$T/resolver.native" < "$T/canonical/compilation-envelope.bin" > "$T/canonical.omgrsw1"
python3 "$FRAME" pack "$T/canonical/compilation-envelope.bin" \
  "$T/canonical.omgrsw1" > "$T/canonical.omglow"
"$T/lowerer.native" < "$T/canonical.omglow" > "$T/native.ckir"
"$T/backend.native" < "$T/native.ckir" > "$T/native.elf"
python3 "$FIXTURE" check-ckir "$T/native.ckir"
python3 "$CKIR_REFERENCE" run "$T/native.ckir" > "$T/native.status"
cmp "$T/canonical/expected-observation.txt" "$T/native.status" >/dev/null
python3 "$ELF_REFERENCE" check "$T/native.ckir" "$T/native.elf"

run_gamma() { # label template input expected-output timeout
  python3 - "$1" "$T/interp.exe" "$2" "$3" "$4" "$5" "$T" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

label, interpreter, template_name, input_name, output_name, timeout_text, temp = sys.argv[1:]
template = Path(template_name).read_text(encoding="ascii")
if template.count("STDIN") != 1:
    raise SystemExit(f"two-package composite meaning FAIL - {label} placeholder count")
stdin = "Nil"
for byte in reversed(Path(input_name).read_bytes()):
    stdin = f"(Cons {byte} {stdin})"
program = template.replace("STDIN", stdin).encode("ascii")
timeout = float(timeout_text)
started = time.monotonic()
print(
    f"two-package composite meaning: START {label} (timeout {timeout:.0f}s)",
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
            f"two-package composite meaning FAIL - {label} exceeded {timeout:.0f}s"
        )
    try:
        stdout, stderr = process.communicate(timeout=min(heartbeat, remaining))
        break
    except subprocess.TimeoutExpired:
        print(
            f"two-package composite meaning: WAIT {label} "
            f"{time.monotonic() - started:.2f}s of {timeout:.0f}s",
            flush=True,
        )
elapsed = time.monotonic() - started
if process.returncode != 0:
    detail = stderr.decode("utf-8", errors="replace")[-1000:]
    raise SystemExit(
        f"two-package composite meaning FAIL - {label} interpreter status "
        f"{process.returncode}: {detail}"
    )
(Path(temp) / f"{label}.observation").write_bytes(stdout)
print(
    f"two-package composite meaning: PASS {label} interpreter in {elapsed:.2f}s",
    flush=True,
)
PY
  STATUS=$(python3 "$DECODER" "$T/$1.observation" "$T/$1.stdout")
  [ "$STATUS" -eq 0 ] || {
    echo "two-package composite meaning FAIL - $1 status $STATUS, expected 0" >&2
    exit 1
  }
  cmp "$T/$1.stdout" "$4" >/dev/null || {
    echo "two-package composite meaning FAIL - $1 published bytes differ" >&2
    exit 1
  }
  echo "two-package composite meaning: PASS $1 => status 0, exact stdout"
}

run_gamma lowerer "$T/lowerer.gamma" "$T/canonical.omglow" "$T/native.ckir" 240
# Deliberately feed the bytes published by Gamma lowerer, not the native copy.
run_gamma backend "$T/backend.gamma" "$T/lowerer.stdout" "$T/native.elf" 240
python3 "$ELF_REFERENCE" check "$T/lowerer.stdout" "$T/backend.stdout"

echo "two-package composite meaning: actual Gamma lowerer CKIR -> Gamma backend ELF agrees exactly with native result 70 and independent reconstruction"
