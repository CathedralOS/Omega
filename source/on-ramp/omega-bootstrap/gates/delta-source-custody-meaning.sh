#!/usr/bin/env sh
# Source-custody frontend meaning probe. Elaborate the raw Delta checker once
# through the Beta-written Delta-to-Gamma route, then require the canonical
# Gamma interpreter to reproduce representative accepted, semantic-rejected,
# and exhausted observations. This is checker cost evidence, not an artifact
# lowering claim.
set -e

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "source-custody meaning: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$GATE_DIR"

command -v python3 >/dev/null 2>&1 || {
  echo "source-custody meaning: python3 required" >&2
  exit 2
}

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-source-custody-check.alp"
ACTUAL="$OMEGA_REPO_ROOT/source/psi/source/source.omg"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
[ -f "$CHECKER" ] || { echo "source-custody meaning: checker source absent" >&2; exit 1; }
[ -f "$ACTUAL" ] || { echo "source-custody meaning: product source fixture absent" >&2; exit 1; }

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null \
  || { echo "source-custody meaning FAIL - Beta compiler artifact" >&2; exit 1; }

build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}

build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" \
  || { echo "source-custody meaning FAIL - omega2gamma build" >&2; exit 1; }
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" \
  || { echo "source-custody meaning FAIL - Gamma interpreter build" >&2; exit 1; }

# Keep elaboration bounded independently of the later executions. The ceiling
# matches the canonical frontend meaning gate, so this focused probe cannot
# normalize unchecked compiler growth merely by being a separate script.
python3 - "$T/elaborate.exe" "$CHECKER" "$T/checker.gamma" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

elaborator, source_name, output_name = sys.argv[1:]
timeout = 20
started = time.monotonic()
print(
    f"source-custody meaning: START checker elaboration (timeout {timeout}s)",
    flush=True,
)
try:
    with open(source_name, "rb") as source, open(output_name, "wb") as output:
        result = subprocess.run(
            [elaborator],
            stdin=source,
            stdout=output,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
except subprocess.TimeoutExpired:
    elapsed = time.monotonic() - started
    raise SystemExit(
        f"source-custody meaning FAIL - checker elaboration exceeded "
        f"{timeout}s after {elapsed:.2f}s"
    )

elapsed = time.monotonic() - started
if result.returncode != 0:
    detail = result.stderr.decode("utf-8", errors="replace")[:240]
    raise SystemExit(
        f"source-custody meaning FAIL - checker elaboration returned "
        f"{result.returncode} after {elapsed:.2f}s: {detail}"
    )
print(f"source-custody meaning: PASS checker elaboration in {elapsed:.2f}s", flush=True)
PY

[ -s "$T/checker.gamma" ] && ! grep -q 'E2G-UNSUPPORTED' "$T/checker.gamma" \
  || { echo "source-custody meaning FAIL - checker elaboration unsupported" >&2; exit 1; }
checker_gamma_bytes=$(wc -c < "$T/checker.gamma" | tr -d ' ')
checker_gamma_ceiling=1048576
[ "$checker_gamma_bytes" -le "$checker_gamma_ceiling" ] || {
  echo "source-custody meaning FAIL - checker Gamma expanded to $checker_gamma_bytes bytes (ceiling $checker_gamma_ceiling)" >&2
  exit 1
}
echo "source-custody meaning: checker Gamma $checker_gamma_bytes bytes (ceiling $checker_gamma_ceiling)"

mkdir "$T/cases"
python3 - "$T/cases" "$ACTUAL" <<'PY'
from pathlib import Path
import sys

out = Path(sys.argv[1])
actual = Path(sys.argv[2]).read_bytes()
rows = []

def case(name, expected, source):
    path = out / f"{name}.omg"
    if isinstance(source, str):
        source = source.encode("ascii")
    path.write_bytes(source)
    rows.append((name, expected, path))

case("actual-source-unit", 0, actual)
case("reject-unguarded-index", 251, r'''
data Buffer { bytes: [u8; 8] in Trapping; length: u32 [0..=8]; }
machine Buffer::bad(&self, at: u32 in Trapping) -> u8 { self.bytes[at] }
''')
case(
    "exhaust-array-length-65537",
    252,
    "data ArrayHost { bytes: [u8; 65537] in Trapping; }",
)

with (out / "manifest.tsv").open("w", encoding="utf-8") as manifest:
    for name, expected, path in rows:
        manifest.write(f"{name}\t{expected}\t{path}\n")
PY

# Each case receives an independent hard timeout, with a START line before the
# interpreter runs and a PASS line carrying elapsed time afterward. A slow or
# stuck case is therefore both bounded and externally visible.
python3 - "$T/interp.exe" "$T/checker.gamma" "$T/cases/manifest.tsv" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

interpreter, template_name, manifest_name = sys.argv[1:]
template = Path(template_name).read_text(encoding="utf-8")
if template.count("STDIN") != 1:
    raise SystemExit(
        "source-custody meaning FAIL - checker Gamma must contain exactly one STDIN placeholder"
    )

rows = []
for line in Path(manifest_name).read_text(encoding="utf-8").splitlines():
    label, expected, source_name = line.split("\t")
    rows.append((label, int(expected), Path(source_name)))

def gamma_list(contents):
    value = "Nil"
    for byte in reversed(contents):
        value = f"(Cons {byte} {value})"
    return value

heartbeat = 15
total = 0.0
slowest = ("", 0.0)
for label, expected, source in rows:
    # The exact product unit is the intentionally compiler-sized observation.
    # Name/order independence is already repeated through native and self-built
    # checkers; lower-rung meaning executes the same checker once plus distinct
    # 251/252 paths instead of adding another two-minute equivalent positive.
    timeout = 150 if label == "actual-source-unit" else 45
    program = template.replace("STDIN", gamma_list(source.read_bytes()))
    started = time.monotonic()
    print(
        f"source-custody meaning: START {label} (timeout {timeout}s)",
        flush=True,
    )
    process = subprocess.Popen(
        [interpreter],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert process.stdin is not None
    process.stdin.write(program.encode("utf-8"))
    process.stdin.close()
    process.stdin = None
    while True:
        remaining = timeout - (time.monotonic() - started)
        if remaining <= 0:
            process.kill()
            process.communicate()
            elapsed = time.monotonic() - started
            raise SystemExit(
                f"source-custody meaning FAIL - {label} exceeded {timeout}s "
                f"after {elapsed:.2f}s"
            )
        try:
            stdout, stderr = process.communicate(timeout=min(heartbeat, remaining))
            break
        except subprocess.TimeoutExpired:
            elapsed = time.monotonic() - started
            print(
                f"source-custody meaning: WAIT {label} {elapsed:.2f}s "
                f"of {timeout}s",
                flush=True,
            )

    elapsed = time.monotonic() - started
    total += elapsed
    if elapsed > slowest[1]:
        slowest = (label, elapsed)
    # omega2gamma specifies a bare status for exit-only programs. The checker
    # has no output boundary, so close that interpreter result with the exact
    # independently known stdout observation Nil before comparing the pair.
    expected_scalar = f"{expected}\n".encode("ascii")
    expected_observation = f"(Pair {expected} Nil)"
    if process.returncode != expected or stdout != expected_scalar:
        stdout_detail = stdout.decode("utf-8", errors="replace")[:240]
        stderr_detail = stderr.decode("utf-8", errors="replace")[:240]
        raise SystemExit(
            f"source-custody meaning FAIL - {label} observed "
            f"process={process.returncode} stdout={stdout_detail!r} "
            f"stderr={stderr_detail!r}; "
            f"expected interpreter scalar {expected_scalar.decode().strip()!r} "
            f"for {expected_observation!r}"
        )
    print(
        f"source-custody meaning: PASS {label} => Pair {expected} Nil "
        f"in {elapsed:.2f}s",
        flush=True,
    )

print(
    f"source-custody meaning: {len(rows)} cases passed in {total:.2f}s; "
    f"slowest {slowest[0]} {slowest[1]:.2f}s"
)
PY
