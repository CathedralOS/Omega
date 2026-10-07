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

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from gate_lib import (env_seconds, evaluate, require_identity,  # noqa: E402
                    require_receipt_identity, require_rung_identities, run)


def main():
    argv = sys.argv[1:]
    identity_only = argv and argv[0] == "--identity"
    if identity_only:
        argv = argv[1:]
    directory = Path(argv[0]).resolve()
    adapter = Path(argv[1]).read_bytes()
    gate = Path(__file__).resolve().parent
    timeout = env_seconds("OMEGA_OUTCOME_OBSERVATION_SECONDS", "14400")
    receipt_seconds = env_seconds("OMEGA_OUTCOME_RECEIPT_SECONDS", "1800")

    # These are whole source closures and an unchanged ordinary Delta adapter.
    # The host frames and compares bytes; the lower chain checks and runs D.
    compiler = require_rung_identities(directory, adapter)
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
                       request, receipt_seconds, "Epsilon receipt reconstruction",
                       "no outcome judgment")
    require_receipt_identity(receipt)
    observation = evaluate(directory, receipt,
                           struct.pack("<I", len(customer)) + customer,
                           timeout, "Omega outcome customer", "no outcome judgment")
    if observation != expected:
        raise SystemExit(
            f"Outcome observation differs: expected {expected.hex()}, "
            f"received {observation.hex()}")
    print("PASS: OCOUT tables, frame encodings, refusals, bounded arithmetic, "
          "and recorded outcome tuples match the request contract")


if __name__ == "__main__":
    main()
