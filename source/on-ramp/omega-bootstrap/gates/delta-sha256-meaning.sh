#!/usr/bin/env sh
# Rust-free one-block SHA-256 known-answer observation. Native compiler and
# self-build comparisons remain suspended pending lower-rooted publication.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "Delta SHA-256 meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
  echo "Delta SHA-256 meaning: python3 required" >&2
  exit 2
}

SOURCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-sha256.alp"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
VECTORS="$GATE_DIR/fixtures/sha256-known-answer/vectors.tsv"
for FILE in "$SOURCE" "$DECODER" "$VECTORS"; do
  [ -f "$FILE" ] || { echo "Delta SHA-256 meaning: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "Delta SHA-256 meaning: Beta compiler artifact unavailable" >&2
  exit 1
}
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe"
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe"

python3 - "$T/elaborate.exe" "$SOURCE" "$T/sha.gamma" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

elaborator, source, output = sys.argv[1:]
started = time.monotonic()
with open(source, "rb") as stdin, open(output, "wb") as stdout:
    try:
        result = subprocess.run(
            [elaborator], stdin=stdin, stdout=stdout, stderr=subprocess.PIPE,
            timeout=30, check=False,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit("Delta SHA-256 meaning: elaboration exceeded 30s")
payload = Path(output).read_bytes()
if result.returncode != 0 or not payload or b"E2G-UNSUPPORTED" in payload:
    raise SystemExit(
        "Delta SHA-256 meaning: unsupported elaboration: "
        + result.stderr.decode("utf-8", errors="replace")[-1000:]
    )
if len(payload) > 131_072:
    raise SystemExit(
        f"Delta SHA-256 meaning: Gamma carrier {len(payload)} exceeds 131072 bytes"
    )
print(
    f"Delta SHA-256 meaning: elaborated {len(payload)} bytes in "
    f"{time.monotonic()-started:.2f}s"
)
PY

python3 - "$VECTORS" "$T/input" "$T/expected" <<'PY'
from pathlib import Path
import sys

vectors_name, input_name, expected_name = sys.argv[1:]
rows = {}
for line in Path(vectors_name).read_text(encoding="ascii").splitlines():
    label, message, digest = line.split("\t")
    rows[label] = (bytes.fromhex(message), bytes.fromhex(digest))
message, expected = rows["abc"]
Path(input_name).write_bytes(message)
Path(expected_name).write_bytes(expected)
PY
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/encode-gamma-input.py" inject \
  "$T/sha.gamma" "$T/input" "$T/program.gamma"

python3 - "$T/interp.exe" "$T/program.gamma" "$T/observation" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

interpreter, program, observation = sys.argv[1:]
started = time.monotonic()
process = subprocess.Popen(
    [interpreter], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
)
assert process.stdin is not None
process.stdin.write(Path(program).read_bytes()); process.stdin.close(); process.stdin = None
timeout = 150.0
heartbeat = 30.0
while True:
    remaining = timeout - (time.monotonic() - started)
    if remaining <= 0:
        process.kill(); process.communicate()
        raise SystemExit("Delta SHA-256 meaning: abc exceeded 150s")
    try:
        stdout, stderr = process.communicate(timeout=min(heartbeat, remaining))
        break
    except subprocess.TimeoutExpired:
        print(
            f"Delta SHA-256 meaning: WAIT abc {time.monotonic()-started:.2f}s/150s",
            flush=True,
        )
if process.returncode != 0:
    raise SystemExit(
        f"Delta SHA-256 meaning: interpreter status {process.returncode}: "
        + stderr.decode("utf-8", errors="replace")[-1000:]
    )
Path(observation).write_bytes(stdout)
print(f"Delta SHA-256 meaning: interpreted abc in {time.monotonic()-started:.2f}s")
PY

STATUS=$(python3 "$DECODER" "$T/observation" "$T/actual")
[ "$STATUS" -eq 0 ] || {
  echo "Delta SHA-256 meaning: decoded status $STATUS, expected 0" >&2
  exit 1
}
cmp "$T/expected" "$T/actual"
echo "Delta SHA-256 meaning: PASS abc exact digest through canonical Gamma"
