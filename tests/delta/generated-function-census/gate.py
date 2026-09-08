"""Compile ordinary Delta function censuses and execute their complete receipts."""

import csv
import hashlib
import os
from pathlib import Path
import struct
import subprocess
import sys
import time


def require_identity(label, data, length, digest):
    actual = len(data), hashlib.sha256(data).hexdigest()
    if actual != (int(length), digest):
        raise SystemExit(f"{label}: identity changed to {actual}")


def positive_timeout(name, default):
    try:
        timeout = int(os.environ.get(name, str(default)))
    except ValueError:
        raise SystemExit(f"{name} must be a positive integer")
    if timeout <= 0:
        raise SystemExit(f"{name} must be a positive integer")
    return timeout


def evaluate(directory, program, sealed_input, timeout, label):
    started = time.monotonic()
    try:
        result = subprocess.run(
            [str(directory / "evaluator")],
            input=struct.pack("<I", len(program)) + program + sealed_input,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit(f"{label}: observation timeout; no language judgment")
    if result.returncode != 0 or result.stderr:
        raise SystemExit(f"{label}: status {result.returncode}, "
                         f"output {result.stdout[:40].hex()}, stderr {result.stderr!r}")
    print(f"{label}: 0, {len(result.stdout)} bytes, empty stderr, "
          f"{time.monotonic() - started:.3f}s", flush=True)
    return result.stdout


def source_fixture(count, width):
    source = b"".join(
        f"(def f{index:0{width}d} () Int 65)\n".encode("ascii")
        for index in range(count - 1)
    )
    return source + (
        f"(def main ((input Bytes)) Bytes (bytes_single (f{count - 2:0{width}d})))\n"
    ).encode("ascii")


def normalization_fixture():
    parameters = b" ".join(f"(p{index:03d} Int)".encode("ascii") for index in range(256))
    child = b"(w " + b" ".join([b"(g (f))"] * 256) + b")"
    # Root w has budget3, child w budget2, and each height2 leaf budget1.
    # Normalization therefore extracts 256*256 ordinary helper definitions.
    body = (b"(if 1 " * 252 + b"(w " + b" ".join([child] * 256) + b")"
            + b" 0)" * 252)
    return (b"(def f () Int 0)\n(def g ((x Int)) Int x)\n"
            + b"(def w (" + parameters + b") Int 0)\n"
            + b"(def deep () Int " + body + b")\n"
            + b"(def main ((input Bytes)) Bytes input)\n")


def main():
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != "--normalization"):
        raise SystemExit("usage: gate.py MATERIALIZED_DIRECTORY [--normalization]")
    normalization_only = len(sys.argv) == 3
    directory = Path(sys.argv[1])
    gate = Path(__file__).resolve().parent
    timeout = positive_timeout("OMEGA_DELTA_CENSUS_SECONDS", 1200)
    compiler = (directory / "compiler.gamma").read_bytes()
    require_identity("Delta compiler", compiler, 158290,
                     "84ab380f0d725061d415efa048db3bd2b7c6fe8a525a70207783cb81ce23dda5")
    with (gate / "fixtures.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["functions", "width", "source_bytes", "source_sha256",
                                "receipt_bytes", "receipt_sha256"]:
            raise SystemExit("Delta census fixture header changed")
        rows = list(reader)
    if [(row["functions"], row["width"]) for row in rows] != [("4090", "4"), ("32768", "5")]:
        raise SystemExit("Delta census fixture inventory changed")
    for row in (() if normalization_only else rows):
        label = f"{row['functions']} authored Delta functions"
        source = source_fixture(int(row["functions"]), int(row["width"]))
        require_identity(label, source, row["source_bytes"], row["source_sha256"])
        request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(source)) + source
        receipt = evaluate(directory, compiler, request, timeout, label + " compile")
        require_identity(label + " receipt", receipt, row["receipt_bytes"], row["receipt_sha256"])
        output = evaluate(directory, receipt, b"", timeout, label + " execute")
        if output != b"A":
            raise SystemExit(f"{label}: expected A, received {output.hex()}")
    source = normalization_fixture()
    require_identity("normalization source", source, 530514,
                     "e087fe2574928d6e2917c7b23d438f770fea841eaeebc1bd38a1bb41ffe0cf1c")
    request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(source)) + source
    receipt = evaluate(directory, compiler, request, timeout, "normalization compile")
    require_identity("normalization receipt", receipt, 3066611,
                     "5950e25a48b36e742e11fff2aa7438c0b6d1239810c8c6572e201571e56363ae")
    payload = b"\x00A\x80\xff"
    if evaluate(directory, receipt, payload, timeout, "normalization execute") != payload:
        raise SystemExit("normalization receipt changed binary input/output")
    print(f"Delta generated function census: {1 if normalization_only else 3} exact receipts execute",
          flush=True)


if __name__ == "__main__":
    main()
