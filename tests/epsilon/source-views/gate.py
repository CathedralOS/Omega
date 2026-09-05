"""Reconstruct the selected receipt, then frame and compare authored controls."""

import csv
import hashlib
import struct
import subprocess
import sys
from pathlib import Path


GATE = Path(__file__).resolve().parent


def evaluate(directory, program, sealed_input):
    return subprocess.run(
        [str(directory / "evaluator")],
        input=struct.pack("<I", len(program)) + program + sealed_input,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=300,
    )


def verify_result(result, status, expected, name):
    if (result.returncode, result.stdout, result.stderr) != (status, expected, b""):
        raise SystemExit(
            f"{name}: expected status {status}, {expected.hex()}; "
            f"got {result.returncode}, {result.stdout.hex()}, {result.stderr!r}"
        )


def check_controls(directory, receipt):
    with (GATE / "fixtures.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["fixture", "bytes", "sha256", "expected_hex"]:
            raise SystemExit("source-view fixture header changed")
        rows = list(reader)
    names = [row["fixture"] for row in rows]
    if (len(names) != 10 or len(set(names)) != 10
            or set(names) != {path.name for path in (GATE / "fixtures").iterdir()}):
        raise SystemExit("source-view fixture inventory changed")

    # Both excluded regions contain forbidden Epsilon source bytes. The suffix
    # can complete boundary tokens/escapes if any lookahead escapes the window.
    prefix = b"\xffheader\x00"
    suffix = b'1"\n{}\xffsealed-stdin\x00'
    for row in rows:
        name = row["fixture"]
        if Path(name).name != name:
            raise SystemExit("fixture is not a local member")
        authored = (GATE / "fixtures" / name).read_bytes()
        if (len(authored), hashlib.sha256(authored).hexdigest()) != (
                int(row["bytes"]), row["sha256"]):
            raise SystemExit(f"{name}: authored identity changed")
        source = bytes.fromhex(authored.decode("ascii")) if name.endswith(".hex") else authored
        expected = bytes.fromhex(row["expected_hex"])
        for mode in range(3):
            request = bytes([mode])
            if mode == 0:
                request += source
            else:
                request += struct.pack("<II", 9 + len(prefix), len(source))
                request += prefix + source + suffix
            verify_result(evaluate(directory, receipt, request), 0, expected,
                          f"{name}, route {mode}")
        print(f"{name}: all three source routes agree with authored expectation", flush=True)

    expected = bytes.fromhex((GATE / "extents_expected.hex").read_text(encoding="ascii"))
    if len(expected) != 50:
        raise SystemExit("extent expectation length changed")
    verify_result(evaluate(directory, receipt, b"\x03ABC"), 0, expected, "14 extents")
    for mode in (4, 5, 6):
        # Z must exist beyond the window: backing bounds alone cannot catch
        # either the past-end index or index zero of the empty window.
        verify_result(evaluate(directory, receipt, bytes([mode]) + b"ABCZ"),
                      249, b"", f"invalid view index, mode {mode}")
    print("Epsilon source views: 30 checks, 14 extents and 3 invalid indexes pass", flush=True)


def main():
    directory = Path(sys.argv[1])
    compiler = (directory / "delta_compiler.gamma").read_bytes()
    subject = ((directory / "epsilon_compiler.delta").read_bytes()
               + (directory / "controls.delta").read_bytes())
    with (GATE / "receipt.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["bytes", "sha256"]:
            raise SystemExit("source-view receipt header changed")
        pins = list(reader)
    if len(pins) != 1:
        raise SystemExit("source views require one exact receipt identity")
    request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject)) + subject
    result = evaluate(directory, compiler, request)
    if result.returncode != 0 or result.stderr:
        raise SystemExit(f"source-view compilation failed: {result.returncode}, {result.stderr!r}")
    receipt = result.stdout
    if (len(receipt), hashlib.sha256(receipt).hexdigest()) != (
            int(pins[0]["bytes"]), pins[0]["sha256"]):
        raise SystemExit("source-view receipt identity changed")
    print(f"Epsilon source views: exact {len(receipt)}-byte receipt reconstructed", flush=True)
    check_controls(directory, receipt)


if __name__ == "__main__":
    main()
