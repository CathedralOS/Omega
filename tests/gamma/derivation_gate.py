"""Shared diagnostic-vector gate for the derivation-* suites: one framed
source through the selected evaluator, exact bytes expected. Each suite's
gate.py supplies its own label words, fixtures, and directory."""

import csv
import hashlib
import struct
import subprocess
import sys
import time
from pathlib import Path


def main(label, result_word, verdict, cases, gate):
    temporary = Path(sys.argv[1])
    source = (temporary / "diagnostic.gamma").read_bytes()
    with (gate / "source.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["lines", "bytes", "sha256"]:
            raise SystemExit(f"{label}: wrong identity header")
        identities = list(reader)
    if len(identities) != 1:
        raise SystemExit(f"{label}: expected one diagnostic source identity")
    identity = identities[0]
    actual = (len(source.splitlines()), len(source), hashlib.sha256(source).hexdigest())
    if actual != (int(identity["lines"]), int(identity["bytes"]), identity["sha256"]):
        raise SystemExit(f"{label}: source identity changed: {actual}")

    names = set()
    for name, request, expected, timeout in cases():
        if name in names:
            raise SystemExit(f"{label}: duplicate fixture {name}")
        names.add(name)
        if 4 + len(source) + len(request) > 137363456:
            raise SystemExit(f"{label} {name}: outside evaluator request")
        framed = struct.pack("<I", len(source)) + source + request
        started = time.monotonic()
        try:
            result = subprocess.run(
                [str(temporary / "evaluator")], input=framed,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout,
            )
        except subprocess.TimeoutExpired:
            raise SystemExit(f"{label} {name}: host timeout {timeout}s; no {result_word} result")
        elapsed = time.monotonic() - started
        if (result.returncode, result.stdout, result.stderr) != (0, expected, b""):
            raise SystemExit(
                f"{label} {name}: expected 0/{expected.hex()}, "
                f"got {result.returncode}/{result.stdout.hex()}, "
                f"stderr={result.stderr!r}, elapsed={elapsed:.3f}s"
            )
        if timeout > 60:
            print(f"{label} {name}: {len(request)} bytes, {elapsed:.3f}s, exact {expected.hex()}", flush=True)
    print(f"{label}: {len(names)} exact diagnostic vectors passed; {verdict}")
