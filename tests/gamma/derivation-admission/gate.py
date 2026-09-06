"""Invoke the selected Gamma program and compare complete diagnostic bytes."""

import csv
import hashlib
import struct
import subprocess
import sys
from pathlib import Path

from fixtures import cases


def main():
    temporary = Path(sys.argv[1])
    gate = Path(__file__).resolve().parent
    source = (temporary / "diagnostic.gamma").read_bytes()
    with (gate / "source.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["lines", "bytes", "sha256"]:
            raise SystemExit("Derivation admission: wrong identity header")
        identities = list(reader)
    if len(identities) != 1:
        raise SystemExit("Derivation admission: expected one diagnostic source identity")
    expected = identities[0]
    actual = (len(source.splitlines()), len(source), hashlib.sha256(source).hexdigest())
    if actual != (int(expected["lines"]), int(expected["bytes"]), expected["sha256"]):
        raise SystemExit(f"Derivation admission: source identity changed: {actual}")

    observations = 0
    fixtures = 0
    for name, request, output, repetitions in cases():
        # This is framing custody, not a second semantic or resource model.
        if 4 + len(source) + len(request) > 16777216:
            raise SystemExit(f"Derivation admission {name}: outside selected evaluator request")
        framed_input = struct.pack("<I", len(source)) + source + request
        for repetition in range(repetitions):
            try:
                result = subprocess.run(
                    [str(temporary / "evaluator")], input=framed_input,
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60,
                )
            except subprocess.TimeoutExpired:
                raise SystemExit(f"Derivation admission {name}: host timeout; no admission result")
            if (result.returncode, result.stdout, result.stderr) != (0, output, b""):
                raise SystemExit(
                    f"Derivation admission {name}/{repetition + 1}: expected diagnostic "
                    f"0/{output.hex()}, got {result.returncode}/{result.stdout.hex()}, "
                    f"stderr={result.stderr!r}"
                )
            observations += 1
        fixtures += 1
    print(f"Derivation admission: {fixtures} vectors, {observations} exact diagnostics passed; no proof verdicts")


if __name__ == "__main__":
    main()
