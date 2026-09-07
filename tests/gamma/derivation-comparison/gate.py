"""Compose exact whole sources and compare authored diagnostic bytes."""

import csv
import hashlib
import struct
import subprocess
import sys
import time
from pathlib import Path

from fixtures import ENTRIES, cases


def programs(gate, implementation):
    with (gate / "source.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["entry", "lines", "bytes", "sha256"]:
            raise SystemExit("Derivation comparison: wrong identity header")
        identities = list(reader)
    if tuple(row["entry"] for row in identities) != ENTRIES:
        raise SystemExit("Derivation comparison: entry identity inventory changed")
    if {path.stem for path in (gate / "entries").glob("*.gamma")} != set(ENTRIES):
        raise SystemExit("Derivation comparison: authored entry inventory changed")
    shared = (gate / "diagnostic.gamma").read_bytes()
    result = {}
    for row in identities:
        name = row["entry"]
        source = shared + (gate / "entries" / (name + ".gamma")).read_bytes() + implementation
        actual = (len(source.splitlines()), len(source), hashlib.sha256(source).hexdigest())
        if actual != (int(row["lines"]), int(row["bytes"]), row["sha256"]):
            raise SystemExit(f"Derivation comparison {name}: source identity changed: {actual}")
        result[name] = source
    return result


def main():
    temporary = Path(sys.argv[1])
    gate = Path(__file__).resolve().parent
    sources = programs(gate, (temporary / "implementation.gamma").read_bytes())
    observations = 0
    names = set()
    for name, entry, request, expected, repetitions, timeout in cases():
        if name in names:
            raise SystemExit(f"Derivation comparison: duplicate fixture {name}")
        names.add(name)
        source = sources[entry]
        if 4 + len(source) + len(request) > 16777216:
            raise SystemExit(f"Derivation comparison {name}: outside evaluator request")
        framed = struct.pack("<I", len(source)) + source + request
        for repetition in range(repetitions):
            started = time.monotonic()
            try:
                result = subprocess.run(
                    [str(temporary / "evaluator")], input=framed,
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout,
                )
            except subprocess.TimeoutExpired:
                raise SystemExit(f"Derivation comparison {name}: host timeout {timeout}s; no comparison result")
            elapsed = time.monotonic() - started
            if (result.returncode, result.stdout, result.stderr) != (0, expected, b""):
                raise SystemExit(
                    f"Derivation comparison {name}/{repetition + 1}: expected 0/{expected.hex()}, "
                    f"got {result.returncode}/{result.stdout.hex()}, "
                    f"stderr={result.stderr!r}, elapsed={elapsed:.3f}s"
                )
            observations += 1
            if repetitions == 1:
                print(f"Derivation comparison {name}: {len(request)} bytes, {elapsed:.3f}s, exact {expected.hex()}", flush=True)
    print(f"Derivation comparison: {len(names)} vectors, {observations} exact diagnostics passed; no proof verdicts")


if __name__ == "__main__":
    main()
