#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta request boundary: skipped (python3 absent)"
    exit 0
}

require_seed_execution_host "Delta request boundary"

REQUEST_BOUNDARY_TMP=$(mktemp -d)
trap 'rm -rf -- "$REQUEST_BOUNDARY_TMP"' EXIT HUP INT TERM
materialize_delta_compiler "$REQUEST_BOUNDARY_TMP/compiler.gamma"
materialize_gamma_evaluator "$REQUEST_BOUNDARY_TMP/evaluator" >/dev/null
materialize_delta_support "$REQUEST_BOUNDARY_TMP/support.bin"

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
    3350, 147840, "fbcb9e17b7ce0c75849136086bc5a4b6df4264054be72b5aae6d70325f9d0929"
):
    raise SystemExit(f"Delta compiler identity changed: {identity}")

# The bound support section: the packed support.gamma.sources members that
# end every sealed compiler input. Its bytes are a custody fixture, not a
# host model of the runtime.
support = (directory / "support.bin").read_bytes()

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
    # A complete sealed input is the exact DCREQ request plus the bound
    # support section.
    return header(length=len(source)) + source + support


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


def support_refusal(name, request, code, coordinate):
    # Space-5 coordinates are absolute sealed-input offsets.
    cases.append((name, request, failure(1, code, coordinate, space=5)))


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
# A complete body with no or partial support section is a space-5 refusal at
# the observed end of input, not a malformed request.
support_refusal("empty declared body with no support section",
                header(), 1, 16)
support_refusal("empty declared body with truncated support section",
                header() + support[:17], 1, 33)
support_refusal("one-byte body with truncated support section",
                header(length=1) + b"\x00" + support[:100], 1, 117)
# A byte after the complete sealed input is still a trailing request byte.
malformed("valid source with trailing byte", framed(identity_source) + b"x",
          16 + len(identity_source) + len(support))

for source in (identity_source, b"; raw Delta is not a request\n" + identity_source):
    malformed("raw source cannot select the diagnostic entry", source, 0)

# A full exact-size request reaches the frontend; its first source byte then
# deliberately fails. This tests request admission, not 4-MiB frontend closure.
exact_body = b"\x00" + b" " * (SOURCE_LIMIT - 1)
cases.append(("full exact source extent reaches frontend", framed(exact_body),
              failure(1, 3, 0, space=1)))
malformed("full exact source extent with trailing byte", framed(exact_body) + b"x",
          16 + SOURCE_LIMIT + len(support))
cases.append((
    "full adjacent source extent fails provision", framed(exact_body + b" "),
    failure(2, 1, 12, SOURCE_LIMIT, SOURCE_LIMIT + 1),
))

# A member whose bytes disagree with the bound table refuses at the member's
# absolute sealed-input offset before any Delta-source phase. Reordering the
# packed members is a first-member mismatch.
support_refusal("corrupt byte-runtime member byte",
                framed(identity_source)[:-len(support)]
                + bytes([support[0] ^ 0xff]) + support[1:],
                2, 16 + len(identity_source))
support_refusal("corrupt adapter member byte",
                framed(identity_source)[:-1] + bytes([support[-1] ^ 0xff]),
                2, 16 + len(identity_source) + 2535)
support_refusal("reordered support members",
                header(length=len(identity_source)) + identity_source
                + support[1464:2535] + support[:1464] + support[2535:],
                2, 16 + len(identity_source))

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
    3068, "da9fe09147bca0388e34281bb03a9c69b5ca0c1718e4c885710f4b9e3e1048f4"
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
