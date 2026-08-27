#!/usr/bin/env python3
"""Focused OMGCOMP1 fixtures for the OMGRSW11 record-array relation."""

from __future__ import annotations

import argparse
import hashlib
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "compiler"))
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402

PACKAGE_KEY = "66" * 32
HEADER = struct.Struct("<8sHHHH36I")

CANONICAL = """module app;
data Observation [copy] {
    tag: u8;
    first: u8;
    second: u8;
    third: u8;
    source: u32 in Trapping;
    start: u64 in Trapping;
    end: u64 in Trapping;
    decoded_start: u64 in Trapping;
    decoded_length: u64 in Trapping;
}
data ObservationStream {
    rows: [Observation; 16384] in Trapping;
    count: u64 [0..=16384];
    last_retained: bool;
}
machine ObservationStream::push(&mut self, tag: u8, first: u8, second: u8, third: u8, source: u32 in Trapping, start: u64 in Trapping, end: u64 in Trapping, decoded_start: u64 in Trapping, decoded_length: u64 in Trapping) {
    self.last_retained = false;
    transition self.count < 16384 { true -> retain() _ -> full() }
    state retain(&mut self) {
        self.rows[self.count].tag = tag;
        self.rows[self.count].first = first;
        self.rows[self.count].second = second;
        self.rows[self.count].third = third;
        self.rows[self.count].source = source;
        self.rows[self.count].start = start;
        self.rows[self.count].end = end;
        self.rows[self.count].decoded_start = decoded_start;
        self.rows[self.count].decoded_length = decoded_length;
        self.count = self.count + 1;
        self.last_retained = true;
    }
    state full(&mut self) { self.last_retained = false; }
}
machine ObservationStream::read_tag(&self, index: u64 in Trapping) -> u8 {
    transition index < self.count { true -> present() _ -> absent() }
    state present(&self) { self.rows[index].tag }
    state absent(&self) { 0 }
}
data Main { stream: ObservationStream; }
machine Main::run(&mut self) -> u8 {
    self.stream.push(70, 1, 2, 3, 4, 5, 6, 7, 8);
    self.stream.read_tag(0)
}
"""


def encode(source: str, *, owner: str = "Main", machine: str = "run") -> bytes:
    packed = bundle.encode([bundle.Entry("observations.omg", source.encode("ascii"))])
    manifest = {
        "target": "linux_x86_64",
        "packages": [{"key": PACKAGE_KEY,
                      "sources": [{"label": "observations.omg", "module": "app"}]}],
        "aliases": [],
        "root": {"package": PACKAGE_KEY, "source": "observations.omg",
                 "owner": owner, "machine": machine},
    }
    return compilation.encode_manifest(manifest, packed)


def renamed() -> str:
    result = CANONICAL
    replacements = {
        "ObservationStream": "Ledger", "Observation": "Sample", "Main": "Driver",
        "rows": "items", "count": "used", "last_retained": "kept",
        "push": "record", "read_tag": "peek", "run": "start_here",
        "tag": "kind", "first": "a", "second": "b", "third": "c",
        "source": "origin", "decoded_start": "cooked_at",
        "decoded_length": "cooked_len", "start": "begin", "end": "finish",
        "index": "position",
    }
    for old in sorted(replacements, key=len, reverse=True):
        result = result.replace(old, replacements[old])
    return result


def reordered() -> str:
    result = CANONICAL
    old = """    tag: u8;
    first: u8;
    second: u8;
    third: u8;
    source: u32 in Trapping;
    start: u64 in Trapping;
    end: u64 in Trapping;
    decoded_start: u64 in Trapping;
    decoded_length: u64 in Trapping;
"""
    new = """    decoded_length: u64 in Trapping;
    source: u32 in Trapping;
    second: u8;
    start: u64 in Trapping;
    tag: u8;
    end: u64 in Trapping;
    first: u8;
    decoded_start: u64 in Trapping;
    third: u8;
"""
    changed = result.replace(old, new)
    if changed == result or changed.index("decoded_length: u64") > changed.index("tag: u8"):
        raise AssertionError("reordered Observation fields were not rebuilt")
    return changed


def declaration_reordered() -> str:
    markers = [
        "data Observation [copy] {", "data ObservationStream {",
        "machine ObservationStream::push", "machine ObservationStream::read_tag",
        "data Main {", "machine Main::run",
    ]
    starts = [CANONICAL.index(marker) for marker in markers]
    module = CANONICAL[:starts[0]]
    chunks = [CANONICAL[starts[i]:starts[i + 1]] for i in range(len(starts) - 1)]
    chunks.append(CANONICAL[starts[-1]:])
    # Main and its entry move ahead of both independently recognized stream
    # machines; the two stream machines are also swapped.
    return module + chunks[4] + chunks[0] + chunks[1] + chunks[5] + chunks[3] + chunks[2]


def matrix(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    positives = {
        "canonical": (CANONICAL, "Main", "run"),
        "renamed": (renamed(), "Driver", "start_here"),
        "reordered": (reordered(), "Main", "run"),
        "declaration-reordered": (declaration_reordered(), "Main", "run"),
        "commented-inert": (CANONICAL.replace("data Main { stream: ObservationStream; }",
            "data Main { stream: ObservationStream; /* irrelevant */ spare: bool; }"), "Main", "run"),
    }
    with (output / "positives.tsv").open("w", encoding="ascii") as manifest:
        for name, (source, owner, machine) in positives.items():
            path = output / f"{name}.omgc"
            path.write_bytes(encode(source, owner=owner, machine=machine))
            (output / f"{name}.omg").write_text(source, encoding="ascii")
            manifest.write(f"{name}\t{path}\n")

    negatives = {
        "wrong-field-type": CANONICAL.replace("source: u32 in Trapping;", "source: u8;", 1),
        "missing-store": CANONICAL.replace("        self.rows[self.count].third = third;\n", ""),
        "computed-index": CANONICAL.replace("self.rows[self.count].tag", "self.rows[self.count + 0].tag"),
        "wrong-guard": CANONICAL.replace("self.count < 16384", "self.count < 16383"),
        "missing-array-policy": CANONICAL.replace("[Observation; 16384] in Trapping", "[Observation; 16384]"),
        "wrong-increment": CANONICAL.replace("self.count + 1", "self.count + 2"),
        "wrong-readback": CANONICAL.replace("push(70,", "push(69,"),
        "duplicate-store": CANONICAL.replace("self.rows[self.count].third = third;",
            "self.rows[self.count].second = third;"),
        "array-too-large": CANONICAL.replace("16384", "16385"),
    }
    with (output / "negatives.tsv").open("w", encoding="ascii") as manifest:
        for name, source in negatives.items():
            path = output / f"{name}.omgc"
            path.write_bytes(encode(source))
            manifest.write(f"{name}\t251\t{path}\n")
        exhausted = output / "input-exhausted.omgc"
        exhausted.write_bytes(b"\0" * 267282)
        manifest.write(f"input-exhausted\t252\t{exhausted}\n")


def inspect(compilation_path: Path, witness_path: Path) -> None:
    comp = compilation_path.read_bytes()
    witness = witness_path.read_bytes()
    if len(witness) != 2172:
        raise ValueError(f"OMGRSW11 length {len(witness)} != 2172")
    values = HEADER.unpack_from(witness)
    if values[:5] != (b"OMGRSWB\0", 11, 0, 0, 160):
        raise ValueError("wrong OMGRSW11 identity")
    words = values[5:]
    expected = (2172, len(comp), 1, 10, 3, 13, 3, 10, 7, 2, 9, 9,
                2, 0, 1, 2, 0, 1, 16384, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1)
    if words[:len(expected)] != expected or any(words[len(expected):]):
        raise ValueError("wrong OMGRSW11 header semantics")
    # Unit extent and dense table IDs are independent structural teeth.
    decoded = compilation.decode(comp)
    if struct.unpack_from("<5I", witness, 160) != (0, 0, 0,
            len(decoded.bundle_entries[0].content), 0):
        raise ValueError("wrong OMGRSW11 unit extent")
    offsets_counts_widths = ((180, 10, 32), (500, 3, 32), (596, 13, 24),
                             (908, 3, 56), (1076, 10, 24), (1316, 7, 40),
                             (1596, 2, 36), (1668, 9, 32), (1956, 9, 24))
    for start, count, width in offsets_counts_widths:
        ids = [struct.unpack_from("<I", witness, start + i * width)[0]
               for i in range(count)]
        if ids != list(range(count)):
            raise ValueError(f"non-dense table at {start}")


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    comp = encode(CANONICAL)
    (output / "canonical.omg").write_text(CANONICAL, encoding="ascii")
    (output / "canonical.omgc").write_bytes(comp)
    (output / "identity.txt").write_text(
        f"omgc_bytes={len(comp)}\nomgc_sha256={hashlib.sha256(comp).hexdigest()}\n",
        encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    for command in ("build", "matrix"):
        item = sub.add_parser(command); item.add_argument("output", type=Path)
    item = sub.add_parser("inspect"); item.add_argument("compilation", type=Path); item.add_argument("witness", type=Path)
    args = parser.parse_args()
    if args.command == "build": build(args.output)
    elif args.command == "matrix": matrix(args.output)
    else: inspect(args.compilation, args.witness)


if __name__ == "__main__":
    main()
