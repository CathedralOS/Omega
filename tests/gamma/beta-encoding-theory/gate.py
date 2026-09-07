"""Invoke pinned Gamma sources and compare finite diagnostic observations."""

import csv
import hashlib
import platform
import struct
import subprocess
import sys
import time
from pathlib import Path

from fixtures import cases
from identity import fixed_identity


def pin(path, fields):
    with path.open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != fields:
            raise SystemExit(f"Beta encoding theory: wrong identity header in {path}")
        rows = list(reader)
    if len(rows) != 1:
        raise SystemExit(f"Beta encoding theory: expected one identity in {path}")
    return rows[0]


def source_identity(name, source, path):
    identity = pin(path, ["lines", "bytes", "sha256"])
    actual = (len(source.splitlines()), len(source), hashlib.sha256(source).hexdigest())
    expected = (int(identity["lines"]), int(identity["bytes"]), identity["sha256"])
    if actual != expected:
        raise SystemExit(f"Beta encoding theory: {name} source identity changed: {actual}")
    print(f"Beta encoding theory {name}: lines={actual[0]} bytes={actual[1]} sha256={actual[2]}", flush=True)


def invoke(evaluator, name, source, request, timeout=600):
    framed = struct.pack("<I", len(source)) + source + request
    if len(framed) > 16777216:
        raise SystemExit(f"Beta encoding theory {name}: outside evaluator request")
    started = time.monotonic()
    try:
        result = subprocess.run(
            [str(evaluator)], input=framed, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit(f"Beta encoding theory {name}: host timeout {timeout}s; no checker result")
    return result, time.monotonic() - started


def require(name, result, status, output):
    if (result.returncode, result.stdout, result.stderr) != (status, output, b""):
        raise SystemExit(
            f"Beta encoding theory {name}: expected status={status}, "
            f"stdout={len(output)} bytes/{hashlib.sha256(output).hexdigest()}, "
            f"got status={result.returncode}, stdout={len(result.stdout)} bytes/"
            f"{hashlib.sha256(result.stdout).hexdigest()}, stderr={result.stderr!r}"
        )


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: gate.py PREPARED_DIRECTORY (producer.gamma, checker.gamma, evaluator)")
    temporary = Path(sys.argv[1]).resolve()
    gate = Path(__file__).resolve().parent
    producer = (temporary / "producer.gamma").read_bytes()
    checker = (temporary / "checker.gamma").read_bytes()
    source_identity("producer", producer, gate / "source.tsv")
    source_identity("checker", checker, gate.parent / "derivation-checking" / "source.tsv")
    print(f"Beta encoding theory host: {platform.system()} {platform.machine()}", flush=True)
    evaluator = temporary / "evaluator"

    # The only positive theory is actual stdout from the selected Gamma source.
    # No host field encoder produces or repairs this section.
    result, elapsed = invoke(evaluator, "emit", producer, b"")
    if result.returncode != 0 or result.stderr:
        raise SystemExit(f"Beta encoding theory emitter failed: {result.returncode}/{result.stderr!r}")
    definitions = result.stdout
    identity = pin(gate / "theory.tsv", ["bytes", "sha256"])
    actual = (len(definitions), hashlib.sha256(definitions).hexdigest())
    if actual != fixed_identity():
        raise SystemExit(f"Beta encoding theory: independent fixed wire identity differs: {actual}")
    if actual != (int(identity["bytes"]), identity["sha256"]):
        raise SystemExit(f"Beta encoding theory: emitted package identity changed: {actual}")
    print(f"Beta encoding theory emitted: bytes={actual[0]} sha256={actual[1]}, {elapsed:.3f}s", flush=True)
    repeated, _ = invoke(evaluator, "repeat_emit", producer, b"")
    require("repeat_emit", repeated, 0, definitions)
    for name, request in (("nul", b"\x00"), ("space", b" "), ("theory_prefix", b"GTH1")):
        refused, elapsed = invoke(evaluator, f"producer_input_{name}", producer, request)
        require(f"producer_input_{name}", refused, 1, b"")
        print(f"Beta encoding theory producer_input_{name}: exact status=1, empty stdout/stderr, {elapsed:.3f}s", flush=True)

    observations = 0
    names = set()
    for name, request, expected in cases(definitions):
        if name in names:
            raise SystemExit(f"Beta encoding theory: duplicate fixture {name}")
        names.add(name)
        result, elapsed = invoke(evaluator, name, checker, request)
        require(name, result, 0, expected)
        observations += 1
        print(f"Beta encoding theory {name}: {len(request)} bytes, exact {expected.hex()}, {elapsed:.3f}s", flush=True)
    print(
        f"Beta encoding theory: two identical emissions, three empty-output producer refusals, "
        f"{observations} exact checker diagnostics; 1024 lexical truths, 256 joins, "
        f"512 splits, 256 composed roundtrips, 13 word emissions, "
        f"512 byte counter helper equations, 19 checked word successors, "
        f"256 nibble comparisons, 12 byte comparisons, 30 word comparisons; no artifact admission",
        flush=True,
    )


if __name__ == "__main__":
    main()
