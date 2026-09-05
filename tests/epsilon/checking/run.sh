#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon checking: skipped (python3 absent)"
    exit 0
}

CHECKING_TMP=$(mktemp -d)
trap 'rm -rf -- "$CHECKING_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$CHECKING_TMP/epsilon_compiler.delta"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$CHECKING_TMP/delta_compiler.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
materialize_gamma_evaluator "$CHECKING_TMP/evaluator" >/dev/null

GATE_DIR="$GATE_DIR" CHECKING_TMP="$CHECKING_TMP" python3 - <<'PY'
import csv
import hashlib
import os
import struct
import subprocess
from pathlib import Path

gate = Path(os.environ["GATE_DIR"])
temporary = Path(os.environ["CHECKING_TMP"])
source = (temporary / "epsilon_compiler.delta").read_bytes()
driver = (gate / "checking_driver.delta").read_bytes()
for name, data, size, digest in (
    ("Epsilon source closure", source, 599543,
     "88cabf8b27105f51a10d46c158eeb679fb864e6442755626c690d78a9ca0393c"),
    ("checking driver", driver, 944,
     "d6a066af55a4e1b6b95e825120b632b177b774a4eab68a6d366d8d18a4c55e5d"),
):
    if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")

fixtures = {}
with (gate / "fixtures.tsv").open(encoding="ascii", newline="") as manifest:
    rows = csv.DictReader(manifest, delimiter="\t")
    if rows.fieldnames != ["fixture", "bytes", "sha256", "expected_hex"]:
        raise SystemExit("checking fixture manifest header changed")
    for row in rows:
        name = row["fixture"]
        if Path(name).name != name or not name.endswith(".epsilon") or name in fixtures:
            raise SystemExit(f"invalid or repeated checking fixture: {name}")
        data = (gate / name).read_bytes()
        if len(data) != int(row["bytes"]) or hashlib.sha256(data).hexdigest() != row["sha256"]:
            raise SystemExit(f"{name} identity changed")
        fixtures[name] = (data, bytes.fromhex(row["expected_hex"]))
if set(fixtures) != {path.name for path in gate.glob("*.epsilon")}:
    raise SystemExit("checking fixture manifest does not cover the exact inventory")

def evaluate(program, sealed_input, timeout=300):
    process = subprocess.run(
        [str(temporary / "evaluator")],
        input=struct.pack("<I", len(program)) + program + sealed_input,
        stdout=subprocess.PIPE, timeout=timeout,
    )
    return process.returncode, process.stdout

subject = source + driver
request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject)) + subject
compiler = (temporary / "delta_compiler.gamma").read_bytes()
status, receipt = evaluate(compiler, request)
digest = hashlib.sha256(receipt).hexdigest()
observed_identity = f"{len(receipt)} bytes, SHA-256 {digest}"
if status != 0:
    raise SystemExit(f"checking driver compilation returned {status}: {observed_identity}")
with (gate / "receipt.tsv").open(encoding="ascii", newline="") as manifest:
    rows = csv.DictReader(manifest, delimiter="\t")
    if rows.fieldnames != ["bytes", "sha256"]:
        raise SystemExit("checking receipt manifest header changed")
    identities = list(rows)
if len(identities) != 1:
    raise SystemExit(f"checking receipt identity is not registered: {observed_identity}")
if len(receipt) != int(identities[0]["bytes"]) or digest != identities[0]["sha256"]:
    raise SystemExit(f"checking receipt identity changed: {observed_identity}")
print(f"Epsilon checking: exact {observed_identity} reconstructed", flush=True)

for name, (fixture, expected) in fixtures.items():
    status, observation = evaluate(receipt, fixture, timeout=120)
    if (status, observation) != (0, expected):
        raise SystemExit(
            f"{name}: expected status 0 and {expected.hex()}, "
            f"received status {status} and {observation.hex()}"
        )
print(f"Epsilon checking: {len(fixtures)} exact judgments pass", flush=True)
PY
