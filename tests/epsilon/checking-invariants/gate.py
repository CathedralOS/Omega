"""Reconstruct and compare source-owned checker-helper observations."""

import csv
import hashlib
import struct
import subprocess
import sys
import time
from pathlib import Path


def evaluate(directory, program, sealed_input):
    started = time.monotonic()
    result = subprocess.run(
        [str(directory / "evaluator")],
        input=struct.pack("<I", len(program)) + program + sealed_input,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=300,
    )
    if result.returncode != 0 or result.stderr:
        raise SystemExit(
            f"checking invariants: status {result.returncode}, "
            f"stdout {result.stdout[:40].hex()}, stderr {result.stderr!r}"
        )
    return result.stdout, time.monotonic() - started


def main():
    gate = Path(__file__).resolve().parent
    directory = Path(sys.argv[1])
    compiler = (directory / "delta_compiler.gamma").read_bytes()
    subject = ((directory / "epsilon_compiler.delta").read_bytes()
               + (directory / "controls.delta").read_bytes())
    expected = bytes.fromhex((gate / "expected.hex").read_text(encoding="ascii"))
    with (gate / "receipt.tsv").open(encoding="ascii", newline="") as manifest:
        rows = csv.DictReader(manifest, delimiter="\t")
        if rows.fieldnames != ["bytes", "sha256"]:
            raise SystemExit("checking invariant receipt header changed")
        identities = list(rows)
    if len(identities) != 1:
        raise SystemExit("checking invariant receipt needs one exact identity")
    request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject)) + subject
    receipt, elapsed = evaluate(directory, compiler, request)
    digest = hashlib.sha256(receipt).hexdigest()
    if (len(receipt) != int(identities[0]["bytes"])
            or digest != identities[0]["sha256"]):
        raise SystemExit(
            f"checking invariant receipt changed: {len(receipt)} bytes, {digest}"
        )
    print(f"Epsilon checking invariants: exact {len(receipt)}-byte receipt "
          f"reconstructed in {elapsed:.3f}s", flush=True)
    observation, elapsed = evaluate(directory, receipt, b"")
    if observation != expected:
        raise SystemExit(f"checking invariants expected {expected.hex()}, "
                         f"received {observation.hex()}")
    print(f"Epsilon checking invariants: two exact helper outcomes pass "
          f"in {elapsed:.3f}s; empty stderr", flush=True)


if __name__ == "__main__":
    main()
