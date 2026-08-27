#!/usr/bin/env sh
# Rust-free meaning probe for the structural compilation-envelope checker.
# This is transport checking only; it does not join resolver authority or
# source semantics and it publishes no bytes.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "compilation envelope meaning: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_PATH_BETA/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
  echo "compilation envelope meaning: python3 required" >&2
  exit 2
}

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-compilation-check.alp"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
[ -f "$CHECKER" ] || { echo "compilation envelope meaning: checker absent" >&2; exit 1; }

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null \
  || { echo "compilation envelope meaning FAIL - Beta compiler artifact" >&2; exit 1; }
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" \
  || { echo "compilation envelope meaning FAIL - omega2gamma build" >&2; exit 1; }
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" \
  || { echo "compilation envelope meaning FAIL - Gamma interpreter build" >&2; exit 1; }

python3 - "$T/elaborate.exe" "$CHECKER" "$T/checker.gamma" <<'PY'
import subprocess, sys, time
elaborator, source, output = sys.argv[1:]
started = time.monotonic()
try:
    with open(source, "rb") as stdin, open(output, "wb") as stdout:
        result = subprocess.run([elaborator], stdin=stdin, stdout=stdout,
                                stderr=subprocess.PIPE, timeout=30, check=False)
except subprocess.TimeoutExpired:
    raise SystemExit("compilation envelope meaning FAIL - elaboration exceeded 30s")
if result.returncode != 0:
    raise SystemExit(
        "compilation envelope meaning FAIL - elaboration: "
        + result.stderr.decode("utf-8", errors="replace")[:240]
    )
print(f"compilation envelope meaning: elaborated in {time.monotonic()-started:.2f}s")
PY
[ -s "$T/checker.gamma" ] && ! grep -q 'E2G-UNSUPPORTED' "$T/checker.gamma" \
  || { echo "compilation envelope meaning FAIL - unsupported checker" >&2; exit 1; }
GAMMA_BYTES=$(wc -c < "$T/checker.gamma" | tr -d ' ')
GAMMA_CEILING=1048576
[ "$GAMMA_BYTES" -le "$GAMMA_CEILING" ] || {
  echo "compilation envelope meaning FAIL - Gamma $GAMMA_BYTES bytes exceeds $GAMMA_CEILING" >&2
  exit 1
}

mkdir "$T/cases"
PYTHONPATH="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER" python3 - "$T/cases" <<'PY'
from pathlib import Path
import struct, sys
import omega_bootstrap_bundle as b
import omega_bootstrap_compilation as c
out = Path(sys.argv[1])
key = lambda n: f"{n:064x}"
bundle = b.encode([b.Entry("main.omg", b"machine Owner::main(&mut self) {}")])
manifest = {
    "target": "linux_x86_64",
    "packages": [{"key": key(1), "sources": [{"label": "main.omg", "module": ""}]}],
    "aliases": [],
    "root": {"package": key(1), "source": "main.omg", "owner": "Owner", "machine": "main"},
}
good = c.encode_manifest(manifest, bundle)
bad = bytearray(good); bad[0] ^= 1
over = bytearray(good); struct.pack_into("<I", over, 32, 17)
rows = [("canonical", 0, good), ("reject-magic", 251, bytes(bad)), ("exhaust-package-count", 252, bytes(over))]
with (out / "manifest.tsv").open("w") as f:
    for name, status, data in rows:
        path = out / (name + ".omgc"); path.write_bytes(data)
        f.write(f"{name}\t{status}\t{path}\n")
PY

python3 - "$T/interp.exe" "$T/checker.gamma" "$T/cases/manifest.tsv" <<'PY'
from pathlib import Path
import subprocess, sys, time
interpreter, gamma_name, manifest = sys.argv[1:]
template = Path(gamma_name).read_text()
if template.count("STDIN") != 1:
    raise SystemExit("compilation envelope meaning FAIL - expected one STDIN placeholder")
def gamma_list(data):
    value = "Nil"
    for byte in reversed(data): value = f"(Cons {byte} {value})"
    return value
total = 0.0
for row in Path(manifest).read_text().splitlines():
    name, expected, source = row.split("\t"); expected = int(expected)
    program = template.replace("STDIN", gamma_list(Path(source).read_bytes()))
    started = time.monotonic()
    try:
        result = subprocess.run([interpreter], input=program.encode(), stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=30, check=False)
    except subprocess.TimeoutExpired:
        raise SystemExit(f"compilation envelope meaning FAIL - {name} exceeded 30s")
    elapsed = time.monotonic() - started; total += elapsed
    scalar = f"{expected}\n".encode()
    if result.returncode != expected or result.stdout != scalar:
        raise SystemExit(
            f"compilation envelope meaning FAIL - {name}: process {result.returncode}, "
            f"observation {result.stdout[:160]!r}"
        )
print(f"compilation envelope meaning: PASS 0/251/252 in {total:.2f}s")
PY
echo "compilation envelope meaning: checker Gamma $GAMMA_BYTES bytes (ceiling $GAMMA_CEILING)"
