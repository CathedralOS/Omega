"""Run the Epsilon array-storage scalability and invariant gates."""

import csv
import hashlib
import os
import struct
import subprocess
import sys
import time
from pathlib import Path


EPSILON_BYTES = 617354
EPSILON_SHA256 = "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e"
DRIVER_BYTES = 2565
DRIVER_SHA256 = "ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38"
RECEIPT_BYTES = 719826
RECEIPT_SHA256 = "dd4985c0eb6e1f30bc2178f90dd30e25ae7b842fb544137f606a44e622000f22"
INVARIANTS_BYTES = 8415
INVARIANTS_SHA256 = "2bc73c60572ddac4ebbfe9b36d4d0d5f44268b56fc0cbafc14dd2a127f947144"
INVARIANTS_RECEIPT_BYTES = 721626
INVARIANTS_RECEIPT_SHA256 = "55560f2c9b7575bc9774416ddbe2c2eb64da8ae8d1ac3ca873f1ba64d887819b"


def identity(data):
    return len(data), hashlib.sha256(data).hexdigest()


def positive_timeout(name, default):
    try:
        value = int(os.environ.get(name, str(default)))
    except ValueError:
        raise SystemExit(f"{name} must be a positive integer")
    if value <= 0:
        raise SystemExit(f"{name} must be a positive integer")
    return value


def evaluate(evaluator, program, sealed_input, timeout):
    return subprocess.run(
        [str(evaluator)],
        input=struct.pack("<I", len(program)) + program + sealed_input,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )


def reconstruct_receipt(evaluator, delta, epsilon, driver, timeout,
                        expected_identity, label):
    subject = epsilon + driver
    request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject)) + subject
    started = time.monotonic()
    result = evaluate(evaluator, delta, request, timeout)
    elapsed = time.monotonic() - started
    if result.returncode != 0 or result.stderr:
        raise SystemExit(
            f"{label} receipt reconstruction: status {result.returncode}, "
            f"stderr={result.stderr!r}"
        )
    receipt = result.stdout
    if identity(receipt) != expected_identity:
        raise SystemExit(
            f"{label} receipt identity changed: {len(receipt)} bytes, "
            f"{hashlib.sha256(receipt).hexdigest()}"
        )
    print(
        f"Epsilon array storage: {label} exact {len(receipt)}-byte receipt "
        f"reconstructed in {elapsed:.3f}s",
        flush=True,
    )
    return receipt


def run_receipt(evaluator, receipt, sealed_input, expected, timeout, label):
    started = time.monotonic()
    result = evaluate(evaluator, receipt, sealed_input, timeout)
    elapsed = time.monotonic() - started
    if result.returncode != 0 or result.stderr or result.stdout != expected:
        raise SystemExit(
            f"{label}: expected outer 0/{expected.hex()}, got "
            f"{result.returncode}/{result.stdout.hex()}, stderr={result.stderr!r}"
        )
    print(
        f"Epsilon array storage: {label}: exact {result.stdout.hex()}, "
        f"empty stderr, {elapsed:.3f}s",
        flush=True,
    )


def main():
    if len(sys.argv) != 4:
        raise SystemExit("usage: gate.py <temporary-directory> <driver> <fixture-dir>")
    temporary = Path(sys.argv[1])
    driver_path = Path(sys.argv[2])
    gate = Path(sys.argv[3])
    receipt_timeout = positive_timeout("OMEGA_EPSILON_RECEIPT_SECONDS", 400)
    fixture_timeout = positive_timeout("OMEGA_EPSILON_ARRAY_SECONDS", 1200)

    epsilon = (temporary / "epsilon_compiler.delta").read_bytes()
    delta = (temporary / "delta_compiler.gamma").read_bytes()
    driver = driver_path.read_bytes()
    evaluator = temporary / "evaluator"
    if identity(epsilon) != (EPSILON_BYTES, EPSILON_SHA256):
        raise SystemExit("Epsilon source closure identity changed")
    if identity(driver) != (DRIVER_BYTES, DRIVER_SHA256):
        raise SystemExit("Epsilon execution driver identity changed")

    with (gate / "fixture.tsv").open(encoding="ascii", newline="") as stream:
        rows = csv.DictReader(stream, delimiter="\t")
        if rows.fieldnames != ["fixture", "bytes", "sha256", "expected_hex"]:
            raise SystemExit("array-storage fixture manifest header changed")
        fixtures = list(rows)
    if len(fixtures) != 1:
        raise SystemExit("array-storage gate requires one exact fixture")
    row = fixtures[0]
    fixture_path = gate / row["fixture"]
    fixture = fixture_path.read_bytes()
    if identity(fixture) != (int(row["bytes"]), row["sha256"]):
        raise SystemExit("array-storage fixture identity changed")
    expected = bytes.fromhex(row["expected_hex"])

    receipt = reconstruct_receipt(
        evaluator, delta, epsilon, driver, receipt_timeout,
        (RECEIPT_BYTES, RECEIPT_SHA256), "ordinary",
    )
    run_receipt(
        evaluator, receipt, struct.pack("<I", len(fixture)) + fixture,
        expected, fixture_timeout, row["fixture"],
    )

    invariant_driver = (gate / "invariants.delta").read_bytes()
    if identity(invariant_driver) != (INVARIANTS_BYTES, INVARIANTS_SHA256):
        raise SystemExit("array-storage invariant driver identity changed")
    invariant_expected = bytes.fromhex(
        (gate / "invariants.expected.hex").read_text(encoding="ascii").strip()
    )
    if invariant_expected != bytes([1]) * 40:
        raise SystemExit("array-storage invariant expected observation changed")
    invariant_receipt = reconstruct_receipt(
        evaluator, delta, epsilon, invariant_driver, receipt_timeout,
        (INVARIANTS_RECEIPT_BYTES, INVARIANTS_RECEIPT_SHA256), "invariant",
    )
    run_receipt(
        evaluator, invariant_receipt, b"", invariant_expected,
        fixture_timeout, "private array-storage invariants",
    )
    print("Epsilon array storage: 1 ordinary and 1 private control passed", flush=True)


if __name__ == "__main__":
    main()
