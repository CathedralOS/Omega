"""Exercise D's OCOUT outcome machinery through the selected evaluator."""

ENTRY_SIZE = 19632
ENTRY_SHA256 = "ce58f84f280c4f7682cb4be3f9db1763a165fb82afffa0cd5da8df413c21fa16"

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


def main():
    argv = sys.argv[1:]
    identity_only = argv and argv[0] == "--identity"
    if identity_only:
        argv = argv[1:]
    directory = Path(argv[0]).resolve()
    adapter = Path(argv[1]).read_bytes()
    gate = Path(__file__).resolve().parent
    timeout = int(os.environ.get("OMEGA_OUTCOME_OBSERVATION_SECONDS", "14400"))
    if timeout <= 0:
        raise SystemExit("OMEGA_OUTCOME_OBSERVATION_SECONDS must be positive")
    receipt_seconds = int(os.environ.get("OMEGA_OUTCOME_RECEIPT_SECONDS", "1800"))
    if receipt_seconds <= 0:
        raise SystemExit("OMEGA_OUTCOME_RECEIPT_SECONDS must be positive")

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
    require_identity("outcome customer entry", entry, ENTRY_SIZE,
                     ENTRY_SHA256)
    customer = compiler + entry
    expected = bytes.fromhex((gate / "expected.hex").read_text(encoding="ascii"))
    print(f"Complete D customer: {len(customer)} bytes, "
          f"SHA-256 {hashlib.sha256(customer).hexdigest()}", flush=True)

    subject = epsilon + adapter
    support = (directory / "support.bin").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + support)
    if identity_only:
        # Host-free leg: every bound identity above is checked — entry and the
        # assembled customer/DCREQ byte streams — and the expected observation
        # decodes; only the evaluator executions need a seed host (macOS arm64,
        # Windows x64, or Linux x86-64).
        (directory / "evaluator.exe").stat()
        print(f"Omega outcome: identity legs green; execution legs need a seed "
              f"host ({len(request)}-byte receipt request, {len(customer)}-byte "
              f"customer, {len(expected)}-byte expected observation)", flush=True)
        return
    receipt = evaluate(directory, (directory / "delta_compiler.gamma").read_bytes(),
                       request, receipt_seconds, "Epsilon receipt reconstruction")
    require_identity("Epsilon execution receipt", receipt, 721484,
                     "71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082")
    observation = evaluate(directory, receipt,
                           struct.pack("<I", len(customer)) + customer,
                           timeout, "Omega outcome customer")
    if observation != expected:
        raise SystemExit(
            f"Outcome observation differs: expected {expected.hex()}, "
            f"received {observation.hex()}")
    print("PASS: OCOUT tables, frame encodings, refusals, bounded arithmetic, "
          "and recorded outcome tuples match the request contract")


if __name__ == "__main__":
    main()
