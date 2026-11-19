"""Reconstruct Epsilon execution and observe an ordinary complete-D customer."""

import hashlib
import os
import struct
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from gate_lib import env_seconds  # noqa: E402


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
    timeout = env_seconds("OMEGA_PARSER_OBSERVATION_SECONDS", "14400")

    # These are whole source closures and an unchanged ordinary Delta adapter.
    # The host frames and compares bytes; the lower chain checks and runs D.
    epsilon = (directory / "epsilon_compiler.delta").read_bytes()
    require_identity("Epsilon", epsilon, 616939,
                     "a2830415a4d98176e9bd97e7f28a9c4512d926fdb5c502d73e19d874ed58ca4c")
    require_identity("execution adapter", adapter, 2150,
                     "ab79416b4278d3b132a4fd432485f1b90e868c0549af96e604e2e8da570d1baa")
    compiler = (directory / "omega_compiler.epsilon").read_bytes()
    require_identity("D", compiler, 838497,
                     "5aac4b30bb4c030c1807e03f690e608358414e25b6cffd4a17472121a7c472e2")
    entry = (gate / "main.epsilon").read_bytes()
    require_identity("parser customer entry", entry, 59962,
                     "857fa18ffc214176022d57c2c5d6d5a506b86a4a25c9540460e4a70e73f603c2")
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
        # executions need a seed host (macOS arm64, Windows x64, or Linux x86-64).
        (directory / "evaluator.exe").stat()
        print(f"Interpreted D parser: identity legs green; "
              f"execution legs need a seed host ({len(request)}-byte receipt "
              f"request, {len(customer)}-byte customer)", flush=True)
        return
    receipt_timeout = env_seconds("OMEGA_PARSER_RECEIPT_SECONDS", "1800")
    receipt = evaluate(directory, delta_compiler,
                       request, receipt_timeout, "Epsilon receipt reconstruction")
    require_identity("Epsilon execution receipt", receipt, 719006,
                     "f368dfc90787025cdde8067ca81b83778ceec47791055320235e89e0d845f340")

    observation = evaluate(directory, receipt,
                           struct.pack("<I", len(customer)) + customer,
                           timeout, "Interpreted D parser")
    if observation != expected:
        raise SystemExit(f"Interpreted D parser: expected {expected.hex()}, "
                         f"received {observation.hex()}")
    print("Interpreted D parser: 77 invocations, exact Exit(0) and A", flush=True)


if __name__ == "__main__":
    main()
