#!/usr/bin/env sh
# Bounded Delta SHA-256 producer: native/self agreement, fixed vectors, exact
# OMGCOMP drift receipt, padding edges, mutation, and resource behavior.
# This gate proves digest consistency only; it grants no package/lock authority.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "Delta SHA-256: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "Delta SHA-256: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "Delta SHA-256: skipped ($TOOL absent)"
    exit 0
  }
done

SOURCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-sha256.alp"
VECTORS="$GATE_DIR/fixtures/sha256-known-answer/vectors.tsv"
FIXTURE_TOOL="$GATE_DIR/two_unit_compilation_fixture.py"
for FILE in "$SOURCE" "$VECTORS" "$FIXTURE_TOOL"; do
  [ -f "$FILE" ] || { echo "Delta SHA-256: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$SOURCE" "$T/sha.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$OMEGA_PATH_DELTA/samples/lowermachine.alp" "$T/lowermachine" >/dev/null

python3 - "$T/lowermachine" "$SOURCE" "$T/sha.self.s" <<'PY'
import subprocess
import sys
import time

compiler, source, output = sys.argv[1:]
started = time.monotonic()
with open(source, "rb") as stdin, open(output, "wb") as stdout:
    try:
        result = subprocess.run(
            [compiler], stdin=stdin, stdout=stdout, stderr=subprocess.PIPE,
            timeout=60, check=False,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit("Delta SHA-256: self compilation exceeded 60 seconds")
if result.returncode != 0:
    raise SystemExit(
        "Delta SHA-256: self compilation failed: "
        + result.stderr.decode("utf-8", errors="replace")[-1000:]
    )
print(f"Delta SHA-256: self compilation {time.monotonic()-started:.2f}s")
PY
clang -arch arm64 -o "$T/sha.self" "$T/sha.self.s"
codesign -f -s - "$T/sha.self" >/dev/null 2>&1

python3 "$FIXTURE_TOOL" build "$T/omgcomp"
python3 - "$VECTORS" "$T" <<'PY'
from pathlib import Path
import hashlib
import sys

vectors, root_name = sys.argv[1:]
root = Path(root_name)
rows = []
for line in Path(vectors).read_text(encoding="ascii").splitlines():
    label, message_hex, digest_hex = line.split("\t")
    message = bytes.fromhex(message_hex)
    expected = bytes.fromhex(digest_hex)
    if hashlib.sha256(message).digest() != expected:
        raise SystemExit(f"Delta SHA-256: corrupt fixed vector {label}")
    source = root / f"{label}.input"
    digest = root / f"{label}.digest"
    source.write_bytes(message); digest.write_bytes(expected)
    rows.append((label, 0, source, digest))

for length in (55, 56, 63, 64, 65):
    message = bytes((index * 37 + 11) % 256 for index in range(length))
    source = root / f"padding-{length}.input"
    digest = root / f"padding-{length}.digest"
    source.write_bytes(message); digest.write_bytes(hashlib.sha256(message).digest())
    rows.append((f"padding-{length}", 0, source, digest))

envelope = root / "omgcomp" / "compilation-envelope.bin"
receipt = root / "omgcomp" / "compilation-envelope.sha256"
expected = bytes.fromhex(receipt.read_text(encoding="ascii").strip())
if hashlib.sha256(envelope.read_bytes()).digest() != expected:
    raise SystemExit("Delta SHA-256: canonical OMGCOMP receipt mismatch")
expected_path = root / "omgcomp.digest"
expected_path.write_bytes(expected)
rows.append(("canonical-omgcomp1", 0, envelope, expected_path))

mutated = bytearray(envelope.read_bytes()); mutated[-1] ^= 1
mutated_path = root / "mutated-omgcomp1.input"
mutated_digest = root / "mutated-omgcomp1.digest"
mutated_path.write_bytes(mutated)
mutated_digest.write_bytes(hashlib.sha256(mutated).digest())
if mutated_digest.read_bytes() == expected:
    raise SystemExit("Delta SHA-256: envelope mutation did not change digest")
rows.append(("mutated-omgcomp1", 0, mutated_path, mutated_digest))

maximum = root / "maximum.input"
maximum_digest = root / "maximum.digest"
maximum.write_bytes(bytes(267_280))
maximum_digest.write_bytes(hashlib.sha256(maximum.read_bytes()).digest())
rows.append(("maximum-267280", 0, maximum, maximum_digest))

over = root / "exhaust-267281.input"
no_output = root / "no-output.digest"
over.write_bytes(bytes(267_281)); no_output.write_bytes(b"")
rows.append(("exhaust-267281", 252, over, no_output))

with (root / "cases.tsv").open("w", encoding="utf-8") as manifest:
    for label, status, source, digest in rows:
        manifest.write(f"{label}\t{status}\t{source}\t{digest}\n")
PY

python3 - "$T/sha.native" "$T/sha.self" "$T/cases.tsv" <<'PY'
from pathlib import Path
import subprocess
import sys
import time

native, self_built, manifest = sys.argv[1:]
rows = [line.split("\t") for line in Path(manifest).read_text().splitlines()]
for implementation in (native, self_built):
    started_all = time.monotonic()
    for label, status_text, input_name, output_name in rows:
        expected_status = int(status_text)
        expected_output = Path(output_name).read_bytes()
        started = time.monotonic()
        with open(input_name, "rb") as stdin:
            try:
                result = subprocess.run(
                    [implementation], stdin=stdin, stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE, timeout=20, check=False,
                )
            except subprocess.TimeoutExpired:
                raise SystemExit(
                    f"Delta SHA-256: {Path(implementation).name}/{label} exceeded 20s"
                )
        if result.returncode != expected_status or result.stdout != expected_output:
            raise SystemExit(
                f"Delta SHA-256: {Path(implementation).name}/{label}: expected "
                f"status {expected_status} and {expected_output.hex()}, got "
                f"status {result.returncode}, {result.stdout.hex()}"
            )
        if label == "maximum-267280":
            print(
                f"Delta SHA-256: {Path(implementation).name} exact maximum "
                f"{time.monotonic()-started:.2f}s"
            )
    print(
        f"Delta SHA-256: {Path(implementation).name} PASS {len(rows)} cases "
        f"in {time.monotonic()-started_all:.2f}s"
    )
PY

echo "Delta SHA-256: PASS structural/digest consistency only"
