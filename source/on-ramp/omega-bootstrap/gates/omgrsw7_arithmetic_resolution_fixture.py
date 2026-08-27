#!/usr/bin/env python3
"""Focused OMGRSW7 source fixtures and positional witness inspection."""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from shared_byte_view_resolution_fixture import encode  # noqa: E402


HEADER = struct.Struct("<8s4H17I")
WIDTHS = (36, 48, 28, 28, 24, 24, 24, 24, 28, 24, 40, 24, 40, 24)
TABLE_NAMES = (
    "units", "imports", "bindings", "declarations", "types", "records", "fields",
    "sums", "cases", "payloads", "machines", "machine_parameters", "blocks", "block_parameters",
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def source(body: bytes, *, fields: bytes | None = None, helper: bytes = b"") -> bytes:
    if fields is None:
        fields = b"left: u32 in Trapping; right: u32 in Trapping; result: u32 in Trapping;"
    authored = (
        b"module app;\ndata Probe { " + fields + b" }\n" + helper
        + b"machine Probe::run(&mut self) -> u8 { " + body + b"; 70 }\n"
    )
    return encode(authored)


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    all_named_leaves = encode(b"""module app;
data Probe { left: u32 in Trapping; result: u32 in Trapping; }
machine Probe::helper(&self, left: u32 in Trapping, right: u32 in Trapping) -> u32 in Trapping {
    left - right
}
machine Probe::run(&mut self) -> u8 {
    self.result = self.left - 1;
    transition { _ -> calculate(9, 6) }
    state calculate(&mut self, left: u32 in Trapping, right: u32 in Trapping) {
        self.result = left * right;
        70
    }
}
""")
    view_plus_arithmetic = encode(b"""module app;
data Probe { left: u32 in Trapping; right: u32 in Trapping; result: u32 in Trapping; }
machine Probe::view(&self, bytes: &[u8]) -> u8 { 0 }
machine Probe::run(&mut self) -> u8 {
    self.view("F");
    self.result = self.left * self.right;
    70
}
""")
    cases: dict[str, tuple[int, bytes, str | None]] = {
        "subtract": (0, source(b"self.result = self.left - self.right"), "OMGRSW7"),
        "multiply": (0, source(b"self.result = self.left * self.right"), "OMGRSW7"),
        "recursive-add": (0, source(b"self.result = (self.left + self.right) + 1"), "OMGRSW7"),
        "maximum-literal": (0, source(b"self.result = self.left - 4294967295"), "OMGRSW7"),
        "arithmetic-with-literal": (0, source(b'self.result = self.left - self.right; "A{B}"'), "OMGRSW7"),
        "precedence": (0, source(b"self.result = self.left + self.right * 2"), "OMGRSW7"),
        "all-named-leaves": (0, all_named_leaves, "OMGRSW7"),
        "view-plus-arithmetic": (0, view_plus_arithmetic, "OMGRSW7"),
        "legacy-leaf-plus-literal": (0, source(b"self.result = self.left + 1"), "OMGRSW1"),
        "legacy-constrained-subtract": (0, source(
            b"self.result = self.left - self.right",
            fields=(b"left: u32 in Trapping [0..=10]; right: u32 in Trapping [0..=10]; "
                    b"result: u32 in Trapping [0..=10];"),
        ), "OMGRSW1"),
        "legacy-nontrapping-subtract": (0, source(
            b"self.result = self.left - self.right",
            fields=b"left: u32; right: u32; result: u32;",
        ), "OMGRSW1"),
        "legacy-unary-minus": (0, source(b"self.result = -self.left"), "OMGRSW1"),
        "legacy-comment-operators": (0, source(b"/* 4294967295 * - */ self.result = self.left + 1"), "OMGRSW1"),
        "legacy-indexed-arithmetic": (0, source(
            b"self.result = self.values[self.left + self.right]",
            fields=b"left: u32 in Trapping; right: u32 in Trapping; result: u32 in Trapping; values: [u32 in Trapping; 2];",
        ), "OMGRSW1"),
        "legacy-call-operand": (0, source(
            b"self.result = self.helper() - self.left",
            helper=b"machine Probe::helper(&self) -> u32 in Trapping { 0 }\n",
        ), "OMGRSW1"),
        "high-outside-arithmetic": (251, source(b"self.result = 4294967295"), None),
        "overflow-literal": (251, source(b"self.result = self.left - 4294967296"), None),
        "high-array-length": (252, source(b"70", fields=b"values: [u8; 4294967295];"), None),
        "quoted-operators-v4": (0, encode(b"""module app;
data Probe {}
machine Probe::view(&self, bytes: &[u8]) -> u8 { 0 }
machine Probe::run(&mut self) -> u8 { "4294967295 * -"; 70 }
"""), "OMGRSW4"),
    }
    rows = []
    for name, (status, envelope, magic) in cases.items():
        (output / f"{name}.omgc").write_bytes(envelope)
        rows.append({"name": name, "status": status, "magic": magic})
    (output / "index.json").write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")


def decode(raw: bytes) -> dict[str, list[bytes]]:
    require(len(raw) >= HEADER.size, "truncated OMGRSW7")
    fixed = HEADER.unpack_from(raw)
    require(fixed[:5] == (b"OMGRSW7\0", 7, 0, 0, 84), "OMGRSW7 fixed header")
    words = fixed[5:]
    require(words[0] == len(raw) and words[-1] == 0, "length/reserved")
    counts = (
        words[1], words[2], words[3], words[4], words[5], words[6], words[7],
        words[12], words[13], words[14], words[8], words[9], words[10], words[11],
    )
    at = 84
    rows: dict[str, list[bytes]] = {}
    for table, (count, width) in enumerate(zip(counts, WIDTHS)):
        end = at + count * width
        require(end <= len(raw), f"table {table} extent")
        rows[TABLE_NAMES[table]] = [raw[at + i * width:at + (i + 1) * width] for i in range(count)]
        at = end
    require(at == len(raw), "exact EOF")
    return rows


def check(path: Path) -> None:
    rows = decode(path.read_bytes())
    types = [struct.unpack("<IBBHIIII", row) for row in rows["types"]]
    selected = [row for row in types if row[1:] == (2, 1, 0, 0, 0, 0, 0xFFFFFFFF)]
    require(len(selected) == 1, f"unique exact full u32 in Trapping row: {types!r}")
    print("OMGRSW7 witness: identity, framing, and exact full-u32 semantic word passed")


def check_links(path: Path) -> None:
    rows = decode(path.read_bytes())
    types = [struct.unpack("<IBBHIIII", row) for row in rows["types"]]
    selected = [row[0] for row in types if row[1:] == (2, 1, 0, 0, 0, 0, 0xFFFFFFFF)]
    require(len(selected) == 1, "one selected full-u32 type")
    expected = selected[0]
    for table in ("fields", "machine_parameters", "block_parameters"):
        require(rows[table], f"nonempty {table}")
        actual = [struct.unpack_from("<I", row, 12)[0] for row in rows[table]]
        require(all(type_id == expected for type_id in actual), f"{table} exact type links: {actual!r}")
    print("OMGRSW7 witness: field, machine-parameter, and named-state type links passed")


def check_view(path: Path) -> None:
    rows = decode(path.read_bytes())
    types = [struct.unpack("<IBBHIIII", row) for row in rows["types"]]
    u8_ids = [row[0] for row in types if row[1:] == (1, 0, 0, 0, 0, 0, 255)]
    require(len(u8_ids) == 1, "one canonical full-u8 element")
    views = [row for row in types if row[1:] == (7, 0, 0, u8_ids[0], 0, 0, 0)]
    require(len(views) == 1, f"one inherited shared-byte-view row: {types!r}")
    parameter_types = [struct.unpack_from("<I", row, 12)[0] for row in rows["machine_parameters"]]
    require(views[0][0] in parameter_types, "view machine parameter retains kind-7 type")
    print("OMGRSW7 witness: inherited shared-byte-view relation retained")


def magic(path: Path, expected: str) -> None:
    require(path.read_bytes()[:8] == expected.encode("ascii") + b"\0", f"expected {expected}")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("build"); p.add_argument("output", type=Path)
    p = sub.add_parser("check"); p.add_argument("witness", type=Path)
    p = sub.add_parser("check-links"); p.add_argument("witness", type=Path)
    p = sub.add_parser("check-view"); p.add_argument("witness", type=Path)
    p = sub.add_parser("magic"); p.add_argument("witness", type=Path); p.add_argument("expected")
    args = parser.parse_args()
    if args.command == "build":
        build(args.output)
    elif args.command == "check":
        check(args.witness)
    elif args.command == "check-links":
        check_links(args.witness)
    elif args.command == "check-view":
        check_view(args.witness)
    else:
        magic(args.witness, args.expected)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, struct.error) as error:
        raise SystemExit(f"OMGRSW7 fixture: {error}")
