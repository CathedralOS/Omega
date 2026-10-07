"""Shared diagnostic-vector gates for the derivation-* suites: framed sources
through the selected evaluator, exact bytes expected. Each suite's gate.py
supplies its own label words, fixtures, source shape, and directory."""

import csv
import hashlib
import struct
import subprocess
import sys
import time
from pathlib import Path


def read_source_identity(label, source, gate):
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


def programs(label, gate, implementation):
    from fixtures import ENTRIES
    with (gate / "source.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["entry", "lines", "bytes", "sha256"]:
            raise SystemExit(f"{label}: wrong identity header")
        identities = list(reader)
    if tuple(row["entry"] for row in identities) != ENTRIES:
        raise SystemExit(f"{label}: entry identity inventory changed")
    if {path.stem for path in (gate / "entries").glob("*.gamma")} != set(ENTRIES):
        raise SystemExit(f"{label}: authored entry inventory changed")
    shared = (gate / "diagnostic.gamma").read_bytes()
    result = {}
    for row in identities:
        name = row["entry"]
        source = shared + (gate / "entries" / (name + ".gamma")).read_bytes() + implementation
        actual = (len(source.splitlines()), len(source), hashlib.sha256(source).hexdigest())
        if actual != (int(row["lines"]), int(row["bytes"]), row["sha256"]):
            raise SystemExit(f"{label} {name}: source identity changed: {actual}")
        result[name] = source
    return result


def run_cases(label, result_word, verdict, rows, temporary):
    names = set()
    for name, source, request, expected, timeout in rows:
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


def main(label, result_word, verdict, cases, gate):
    temporary = Path(sys.argv[1])
    source = (temporary / "diagnostic.gamma").read_bytes()
    read_source_identity(label, source, gate)
    run_cases(
        label, result_word, verdict,
        ((name, source, request, expected, timeout)
         for name, request, expected, timeout in cases()), temporary)


def main_entries(label, result_word, verdict, cases, gate):
    temporary = Path(sys.argv[1])
    sources = programs(
        label, gate, (temporary / "implementation.gamma").read_bytes())
    run_cases(
        label, result_word, verdict,
        ((name, sources[entry], request, expected, timeout)
         for name, entry, request, expected, timeout in cases()), temporary)
