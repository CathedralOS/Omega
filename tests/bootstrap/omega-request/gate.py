"""Drive D's canonical OCREQ request entry through the selected evaluator."""

ENTRY_SIZE = 4115
ENTRY_SHA256 = "0d612813e17cfbe2e755b7398d90bb3572f5ed32da249c8863b37f545d3822c0"
REQUEST_SIZE = 132
REQUEST_SHA256 = "ab2e980a89d20651b69782446cd8a8333313dce109636fd3e26cc7f52bc98062"

import hashlib
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import time


def run(command, payload, limit, label):
    started = time.monotonic()
    print(f"{label}: started, {limit}s watchdog", flush=True)
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        output, errors = process.communicate(payload, timeout=limit)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], check=False)
        else:
            os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"{label}: timed out; no outcome judgment")
    print(f"{label}: status {process.returncode}, {time.monotonic() - started:.3f}s", flush=True)
    return process.returncode, output, errors


def evaluate(directory, program, sealed_input, limit, label):
    code, output, errors = run([str(directory / "evaluator.exe")],
                              struct.pack("<I", len(program)) + program + sealed_input,
                              limit, label)
    if code != 0 or errors:
        raise SystemExit(f"{label}: evaluator failure {code}, {output[:40].hex()}, {errors!r}")
    return output


def require_identity(label, source, length, digest):
    actual = hashlib.sha256(source).hexdigest()
    if len(source) != length or actual != digest:
        raise SystemExit(
            f"{label} identity changed: {len(source)} bytes, SHA-256 {actual}")


# The OCOUT V1 failure frame, computed from the assigned tables rather than
# captured output: 8-byte identity, tag, space, two reserved bytes, u32 code,
# then u64 coordinate, limit, and requested amounts.
def frame(tag, space, code, coordinate, limit, requested):
    return (b"\xffOCOUT\x01\x00" + bytes([tag, space, 0, 0])
            + struct.pack("<I", code) + struct.pack("<q", coordinate)
            + struct.pack("<q", limit) + struct.pack("<q", requested))


# The interpreter tapes a controlled exit as a zero status byte plus the
# program's i32 exit code before the program's own stdout bytes.
def observation(code, payload):
    return b"\x00" + struct.pack("<i", code) + payload


def main():
    argv = sys.argv[1:]
    identity_only = argv and argv[0] == "--identity"
    if identity_only:
        argv = argv[1:]
    directory = Path(argv[0]).resolve()
    adapter = Path(argv[1]).read_bytes()
    gate = Path(__file__).resolve().parent
    timeout = int(os.environ.get("OMEGA_REQUEST_OBSERVATION_SECONDS", "14400"))
    if timeout <= 0:
        raise SystemExit("OMEGA_REQUEST_OBSERVATION_SECONDS must be positive")
    receipt_seconds = int(os.environ.get("OMEGA_REQUEST_RECEIPT_SECONDS", "1800"))
    if receipt_seconds <= 0:
        raise SystemExit("OMEGA_REQUEST_RECEIPT_SECONDS must be positive")

    # These are whole source closures and an unchanged ordinary Delta adapter.
    # The host frames and compares bytes; the lower chain checks and runs D.
    epsilon = (directory / "epsilon_compiler.delta").read_bytes()
    require_identity("Epsilon", epsilon, 617354,
                     "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e")
    require_identity("execution adapter", adapter, 2565,
                     "ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38")
    compiler = (directory / "omega_compiler.epsilon").read_bytes()
require_identity("D", compiler, 569920,
                     "f5f051fba1ac62322cc1b0af9f3dc8e5fb1951feef24e44a627f1d9e4c28f842")
    entry = (gate / "main.epsilon").read_bytes()
    require_identity("request entry", entry, ENTRY_SIZE, ENTRY_SHA256)
    fixture = (gate / "request.bin").read_bytes()
    require_identity("canonical request", fixture, REQUEST_SIZE, REQUEST_SHA256)
    customer = compiler + entry
    expected = bytes.fromhex((gate / "expected.hex").read_text(encoding="ascii"))
    print(f"Request customer: {len(customer)} bytes, "
          f"SHA-256 {hashlib.sha256(customer).hexdigest()}", flush=True)

    subject = epsilon + adapter
    support = (directory / "support.bin").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + support)
    if identity_only:
        # Host-free leg: every bound identity above is checked — entry, canonical
        # request fixture, and the assembled customer/DCREQ byte streams — and the
        # expected observation decodes; only the evaluator executions need a seed
        # host (macOS arm64 or Windows x64).
        (directory / "evaluator.exe").stat()
        print(f"Omega request: identity legs green; execution legs need a seed "
              f"host ({len(request)}-byte receipt request, {len(customer)}-byte "
              f"customer, {len(expected)}-byte expected observation)", flush=True)
        return
    receipt = evaluate(directory, (directory / "delta_compiler.gamma").read_bytes(),
                       request, receipt_seconds, "Epsilon receipt reconstruction")
    require_identity("Epsilon execution receipt", receipt, 721484,
                     "71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082")

    def serve(ocreq, label):
        return evaluate(directory, receipt,
                        struct.pack("<I", len(customer)) + customer + ocreq,
                        timeout, label)

    seen = serve(fixture, "Canonical OCREQ request")
    if seen != expected:
        raise SystemExit(
            f"Request observation differs: expected {expected.hex()}, "
            f"received {seen.hex()}")
    print("PASS: canonical request publishes the coverage_request_semantics "
          "frame through the sealed boundary", flush=True)

    # Every refusal is computed from the assigned outcome tables, not
    # captured: each mutated or oversized stream maps to exactly one
    # malformed_request or staging frame anchored at the named byte.
    cases = [
        ("empty request", b"",
         observation(1, frame(1, 1, 1, 0, 0, 0))),
        ("corrupt identity byte", b"N" + fixture[1:],
         observation(1, frame(1, 1, 1, 0, 0, 0))),
        ("truncated invocation", fixture[:100],
         observation(1, frame(1, 1, 1, 12, 0, 0))),
        ("trailing request byte", fixture + b"\x00",
         observation(1, frame(1, 1, 1, 132, 0, 0))),
        ("unassigned product tag", fixture[:73] + b"\x05" + fixture[74:],
         observation(1, frame(1, 1, 1, 73, 0, 0))),
        ("hostile package-name length", fixture[:20] + b"\xff\xff\xff\x7f" + fixture[24:],
         observation(1, frame(1, 1, 1, 20, 0, 0))),
        ("well-formed zero commitment",
         fixture[:-32] + b"\x00" * 32,
         observation(2, frame(2, 0, 25, 0, 0, 0))),
        ("request beyond staging bound", b"A" * 65537,
         observation(2, frame(2, 1, 26, 65536, 65536, 65537))),
    ]
    for name, payload, want in cases:
        seen = serve(payload, f"Request refusal: {name}")
        if seen != want:
            raise SystemExit(
                f"{name}: expected {want.hex()}, received {seen.hex()}")
    print("PASS: every request-boundary refusal publishes its assigned "
          "malformed_request or staging outcome", flush=True)


if __name__ == "__main__":
    main()
