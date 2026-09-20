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
    argv = sys.argv[1:]
    identity_only = argv and argv[0] == "--identity"
    if identity_only:
        argv = argv[1:]
    directory = Path(argv[0])
    adapter = Path(argv[1]).read_bytes()
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
    require_identity("D", compiler, 558161,
                     "8f0321344c893c3c64bb631bfde10e3ecbde4384e130dca9f7e2c21818a9eca3")
    entry = (gate / "main.epsilon").read_bytes()
    require_identity("parser customer entry", entry, 4583,
                     "61f988109564e8ca58d6590941aa1aba3dfc2f07af101fb082b38ff25623e618")
    customer = compiler + entry
    expected = bytes.fromhex((gate / "expected.hex").read_text(encoding="ascii"))
    print(f"Complete D customer: {len(customer)} bytes, "
          f"SHA-256 {hashlib.sha256(customer).hexdigest()}", flush=True)

    subject = epsilon + adapter
    delta_compiler = (directory / "delta_compiler.gamma").read_bytes()
    support = (directory / "support.bin").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + support)
    if identity_only:
        # Host-free leg: every bound identity above is checked and the request
        # and customer byte streams are fully assembled; only the evaluator
        # executions need a seed host (macOS arm64 or Windows x64).
        (directory / "evaluator.exe").stat()
        print(f"Interpreted D parser: identity legs green; "
              f"execution legs need a seed host ({len(request)}-byte receipt "
              f"request, {len(customer)}-byte customer)", flush=True)
        return
    receipt_timeout = int(os.environ.get("OMEGA_PARSER_RECEIPT_SECONDS", "300"))
    if receipt_timeout <= 0:
        raise SystemExit("OMEGA_PARSER_RECEIPT_SECONDS must be positive")
    receipt = evaluate(directory, delta_compiler,
                       request, receipt_timeout, "Epsilon receipt reconstruction")
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
