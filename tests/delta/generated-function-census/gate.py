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


def main():
    directory = Path(sys.argv[1])
    gate = Path(__file__).resolve().parent
    timeout = positive_timeout("OMEGA_DELTA_CENSUS_SECONDS", 1200)
    compiler = (directory / "compiler.gamma").read_bytes()
    require_identity("Delta compiler", compiler, 154972,
                     "1d9688add5cb7c752754dd50d134a90bcace1cc2ccbb23f739498c498a6b0f98")
    with (gate / "fixtures.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["functions", "width", "source_bytes", "source_sha256",
                                "receipt_bytes", "receipt_sha256"]:
            raise SystemExit("Delta census fixture header changed")
        rows = list(reader)
    if [(row["functions"], row["width"]) for row in rows] != [("4090", "4"), ("32768", "5")]:
        raise SystemExit("Delta census fixture inventory changed")
    for row in rows:
        label = f"{row['functions']} authored Delta functions"
        source = source_fixture(int(row["functions"]), int(row["width"]))
        require_identity(label, source, row["source_bytes"], row["source_sha256"])
        request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(source)) + source
        receipt = evaluate(directory, compiler, request, timeout, label + " compile")
        require_identity(label + " receipt", receipt, row["receipt_bytes"], row["receipt_sha256"])
        output = evaluate(directory, receipt, b"", timeout, label + " execute")
        if output != b"A":
            raise SystemExit(f"{label}: expected A, received {output.hex()}")
    print("Delta generated function census: two exact receipts execute", flush=True)


if __name__ == "__main__":
    main()
