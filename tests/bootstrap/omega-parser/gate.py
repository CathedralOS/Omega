"""Reconstruct Epsilon execution and observe an ordinary complete-D customer."""

import hashlib
import os
import struct
import subprocess
import sys
import time
from pathlib import Path


def evaluate(directory, program, sealed_input, timeout, label):
    started = time.monotonic()
    print(f"{label}: started; observation allowance {timeout}s", flush=True)
    try:
        result = subprocess.run(
            [str(directory / "evaluator.exe")],
            input=struct.pack("<I", len(program)) + program + sealed_input,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit(f"{label}: observation timed out; no language judgment")
    elapsed = time.monotonic() - started
    if result.returncode != 0 or result.stderr:
        raise SystemExit(
            f"{label}: status {result.returncode} after {elapsed:.3f}s, "
            f"stdout {result.stdout[:40].hex()}, stderr {result.stderr!r}"
        )
    print(f"{label}: status 0, empty stderr, {elapsed:.3f}s", flush=True)
    return result.stdout


def require_identity(label, source, length, digest):
    actual = hashlib.sha256(source).hexdigest()
    if (len(source), actual) != (length, digest):
        raise SystemExit(f"{label}: identity changed to {len(source)} bytes, {actual}")


def main():
    directory = Path(sys.argv[1])
    adapter = Path(sys.argv[2]).read_bytes()
    gate = Path(__file__).resolve().parent
    timeout = int(os.environ.get("OMEGA_PARSER_OBSERVATION_SECONDS", "14400"))
    if timeout <= 0:
        raise SystemExit("OMEGA_PARSER_OBSERVATION_SECONDS must be positive")

    # These are whole source closures and an unchanged ordinary Delta adapter.
    # The host frames and compares bytes; the lower chain checks and runs D.
    epsilon = (directory / "epsilon_compiler.delta").read_bytes()
    require_identity("Epsilon", epsilon, 617354,
                     "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e")
    require_identity("execution adapter", adapter, 2565,
                     "ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38")
    compiler = (directory / "omega_compiler.epsilon").read_bytes()
    require_identity("D", compiler, 525334,
                     "b507fb785ea450409f3cd1c34f3c451656a124d23ecd912f9e8f8e6e45f41c64")
    entry = (gate / "main.epsilon").read_bytes()
    require_identity("parser customer entry", entry, 4583,
                     "61f988109564e8ca58d6590941aa1aba3dfc2f07af101fb082b38ff25623e618")
    customer = compiler + entry
    expected = bytes.fromhex((gate / "expected.hex").read_text(encoding="ascii"))
    print(f"Complete D customer: {len(customer)} bytes, "
          f"SHA-256 {hashlib.sha256(customer).hexdigest()}", flush=True)

    subject = epsilon + adapter
    support = (directory / "support.bin").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + support)
    receipt = evaluate(directory, (directory / "delta_compiler.gamma").read_bytes(),
                       request, 300, "Epsilon receipt reconstruction")
    require_identity("Epsilon execution receipt", receipt, 721484,
                     "71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082")

    observation = evaluate(directory, receipt,
                           struct.pack("<I", len(customer)) + customer,
                           timeout, "Interpreted D parser")
    if observation != expected:
        raise SystemExit(f"Interpreted D parser: expected {expected.hex()}, "
                         f"received {observation.hex()}")
    print("Interpreted D parser: 12 invocations, exact Exit(0) and A", flush=True)


if __name__ == "__main__":
    main()
