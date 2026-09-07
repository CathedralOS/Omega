"""Frame exact Gamma inputs and compare source-owned ground diagnostics."""

import csv
import hashlib
import struct
import subprocess
import sys
import time
from pathlib import Path

from fixtures import cases


def main():
    temporary = Path(sys.argv[1])
    gate = Path(__file__).resolve().parent
    source = (temporary / "diagnostic.gamma").read_bytes()
    with (gate / "source.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["lines", "bytes", "sha256"]:
            raise SystemExit("Derivation ground: wrong identity header")
        identities = list(reader)
    if len(identities) != 1:
        raise SystemExit("Derivation ground: expected one diagnostic source identity")
    identity = identities[0]
    actual = (len(source.splitlines()), len(source), hashlib.sha256(source).hexdigest())
    if actual != (int(identity["lines"]), int(identity["bytes"]), identity["sha256"]):
        raise SystemExit(f"Derivation ground: source identity changed: {actual}")

    observations = 0
    names = set()
    for name, request, expected, repetitions, timeout in cases():
        if name in names:
            raise SystemExit(f"Derivation ground: duplicate fixture {name}")
        names.add(name)
        if 4 + len(source) + len(request) > 16777216:
            raise SystemExit(f"Derivation ground {name}: outside evaluator request")
        framed = struct.pack("<I", len(source)) + source + request
        for repetition in range(repetitions):
            started = time.monotonic()
            try:
                result = subprocess.run(
                    [str(temporary / "evaluator")], input=framed,
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout,
                )
            except subprocess.TimeoutExpired:
                raise SystemExit(f"Derivation ground {name}: host timeout {timeout}s; no ground result")
            elapsed = time.monotonic() - started
            if (result.returncode, result.stdout, result.stderr) != (0, expected, b""):
                raise SystemExit(
                    f"Derivation ground {name}/{repetition + 1}: expected 0/{expected.hex()}, "
                    f"got {result.returncode}/{result.stdout.hex()}, "
                    f"stderr={result.stderr!r}, elapsed={elapsed:.3f}s"
                )
            observations += 1
            if repetitions == 1:
                print(f"Derivation ground {name}: {len(request)} bytes, {elapsed:.3f}s, exact {expected.hex()}", flush=True)
    print(f"Derivation ground: {len(names)} vectors, {observations} exact diagnostics passed; no proof verdicts")


if __name__ == "__main__":
    main()
