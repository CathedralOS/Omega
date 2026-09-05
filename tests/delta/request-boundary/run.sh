#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta request boundary: skipped (python3 absent)"
    exit 0
}

REQUEST_BOUNDARY_TMP=$(mktemp -d)
trap 'rm -rf -- "$REQUEST_BOUNDARY_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$REQUEST_BOUNDARY_TMP/compiler.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
materialize_gamma_evaluator "$REQUEST_BOUNDARY_TMP/evaluator" >/dev/null

REQUEST_BOUNDARY_TMP="$REQUEST_BOUNDARY_TMP" python3 - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path

directory = Path(os.environ["REQUEST_BOUNDARY_TMP"])
compiler = (directory / "compiler.gamma").read_bytes()
identity = (
    len(compiler.splitlines()), len(compiler), hashlib.sha256(compiler).hexdigest()
)
if identity != (
    2910, 125850, "06fe13c5b046bbc4c9090e27b6366451108d14fca9a37f826e672a7d3b96402c"
):
    raise SystemExit(f"Delta compiler identity changed: {identity}")

REQUEST_MAGIC = b"DCREQ\x01\x00\x00"
OUTCOME_MAGIC = b"\xffDCOUT\x01\x00"
SOURCE_LIMIT = 4194304
identity_source = b"(def main ((source Bytes)) Bytes source)\n"


def evaluate(program, sealed_input):
    request = struct.pack("<I", len(program)) + program + sealed_input
    process = subprocess.Popen(
        [str(directory / "evaluator")], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, error = process.communicate(request, timeout=30)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("Delta request boundary: selected Gamma timed out")
    if error:
        raise SystemExit(f"unexpected evaluator stderr: {error!r}")
    return process.returncode, output


def header(profile=1, length=0):
    return REQUEST_MAGIC + struct.pack("<II", profile, length)


def framed(source):
    return header(length=len(source)) + source


def failure(tag, code, coordinate, limit=0, requested=0, space=4):
    # Common D13/D30 layout, with D33's DCREQ coordinate space and ordering.
    frame = struct.pack(
        "<8sBBHIQQQ", OUTCOME_MAGIC, tag, space, 0, code, coordinate, limit, requested
    )
    assert len(frame) == 40
    return tag, frame


cases = []


def malformed(name, request, coordinate):
    cases.append((name, request, failure(1, 1, coordinate)))


complete_header = header()
for length in range(16):
    malformed(f"header truncation {length}", complete_header[:length], length)
# Obtain the complete fixed header before interpreting even an incorrect byte 0.
malformed("truncated incorrect header", b"X" * 15, 15)

for index in range(8):
    changed = bytearray(header(profile=0, length=0xffffffff))
    changed[index] ^= 0xff
    malformed(f"header byte {index} before profile and provision", bytes(changed), index)

changed = bytearray(header(profile=0, length=0xffffffff))
changed[1] ^= 0xff
changed[7] ^= 0xff
malformed("first of two incorrect header bytes", bytes(changed), 1)

for profile in (0, 2, 3, 256, 65536, 16777216, 0xffffffff):
    cases.append((
        f"unknown profile {profile} before oversized missing body",
        header(profile=profile, length=0xffffffff), failure(1, 2, 8),
    ))
cases.append(("unknown profile before trailing body", header(profile=2) + b"x",
              failure(1, 2, 8)))

for length in (SOURCE_LIMIT + 1, 0x80000000, 0xffffffff):
    for body in (b"", b"\x00extra"):
        cases.append((
            f"source provision {length} with body length {len(body)}",
            header(length=length) + body,
            failure(2, 1, 12, SOURCE_LIMIT, length),
        ))

# The exact declared maximum passes provision and reaches body validation.
malformed("exact provision before missing body", header(length=SOURCE_LIMIT), 16)
malformed("one-byte body missing", header(length=1), 16)
malformed("body truncation before source validation", header(length=4) + b"\x00ab", 19)
malformed("empty declared body with trailing byte", header() + b"\x00", 16)
malformed("first trailing body byte", header(length=1) + b"\x00xy", 17)
malformed("valid source with trailing byte", framed(identity_source) + b"x",
          16 + len(identity_source))

for source in (identity_source, b"; raw Delta is not a request\n" + identity_source):
    malformed("raw source cannot select the diagnostic entry", source, 0)

# A full exact-size request reaches the frontend; its first source byte then
# deliberately fails. This tests request admission, not 4-MiB frontend closure.
exact_body = b"\x00" + b" " * (SOURCE_LIMIT - 1)
cases.append(("full exact source extent reaches frontend", framed(exact_body),
              failure(1, 3, 0, space=1)))
malformed("full exact source extent with trailing byte", framed(exact_body) + b"x",
          16 + SOURCE_LIMIT)
cases.append((
    "full adjacent source extent fails provision", framed(exact_body + b" "),
    failure(2, 1, 12, SOURCE_LIMIT, SOURCE_LIMIT + 1),
))

# Source-envelope and accepted-frontend schema judgments have their own DCOUT
# reasons and source coordinates, separate from DCREQ admission coordinates.
cases.append(("invalid source byte", framed(b"\x00"), failure(1, 3, 0, space=1)))
cases.append(("wrong entry schema", framed(b"(def main () Int 7)\n"),
              failure(1, 20, 5, space=1)))
cases.append(("empty source", framed(b""), failure(1, 4, 0, space=1)))
cases.append(("unmatched opening delimiter", framed(b"("),
              failure(1, 4, 1, space=1)))

# Body-name resolution owns its exact source-coordinate rejection.
unknown_local_prefix = b"(def main ((source Bytes)) Bytes "
cases.append(("unknown local", framed(unknown_local_prefix + b"missing)\n"),
              failure(1, 14, len(unknown_local_prefix), space=1)))

for name, request, expected in cases:
    actual = evaluate(compiler, request)
    if actual != expected:
        raise SystemExit(
            f"{name}: expected status {expected[0]} and {expected[1].hex()}, "
            f"got status {actual[0]}, {len(actual[1])} bytes, prefix {actual[1][:80].hex()}"
        )

status, receipt = evaluate(compiler, framed(identity_source))
receipt_identity = (len(receipt), hashlib.sha256(receipt).hexdigest())
if status != 0 or receipt_identity != (
    1410, "d077f379142c5d4501e029e7b13bca7308c572f9d39e24ef02ffda06581b67d4"
):
    raise SystemExit(f"accepted request changed its exact receipt: {status}, {receipt_identity}")
if evaluate(compiler, framed(identity_source)) != (0, receipt):
    raise SystemExit("accepted request did not reconstruct the same receipt")
for payload in (b"", b"ABC", bytes(range(256))):
    if evaluate(receipt, payload) != (0, payload):
        raise SystemExit("accepted ConformanceBytesV1 receipt changed exact input bytes")

frames = sum(len(expected[1]) == 40 for _, _, expected in cases)
print(
    f"Delta request boundary: {frames} exact DCOUT controls, "
    f"{len(cases) - frames} evaluator-owned failures, "
    "2 identical compilations, and 3 application observations passed"
)
PY
