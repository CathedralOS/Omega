#!/usr/bin/env python3
"""Black-box checks for the product compiler's OCREQ V1 request boundary.

Each request goes to the product on stdin; it must publish exactly the
expected OCOUT V1 frame and exit with the outcome's tag. The first vectors are
D's request-gate vectors (tests/bootstrap/omega-request/gate.py), so both
compilers answer the same requests with the same bytes. The rest are built by
a small field encoder, so every anchor is computed from the position of the
field it names rather than written out by hand.
"""

from __future__ import annotations

import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CANONICAL = (ROOT / "tests/bootstrap/omega-request/request.bin").read_bytes()

REJECT, INCOMPLETE = 1, 2
MALFORMED_REQUEST = 1
COVERAGE_REQUEST_SEMANTICS, REQUEST_STAGING_BYTES = 25, 26
SPACE_NONE, SPACE_REQUEST = 0, 1
HEADER = 16

CASES: list[str] = []


def frame(tag: int, space: int, code: int, coordinate: int = 0, limit: int = 0, requested: int = 0) -> bytes:
    """The 40-byte OCOUT V1 failure frame, from the assigned tables."""
    return (b"\xffOCOUT\x01\x00" + bytes([tag, space, 0, 0]) + struct.pack("<I", code)
            + struct.pack("<QQQ", coordinate, limit, requested))


def malformed(offset: int) -> bytes:
    return frame(REJECT, SPACE_REQUEST, MALFORMED_REQUEST, offset)


# A well-formed request: its semantic phases are the named provision.
SEMANTICS = frame(INCOMPLETE, SPACE_NONE, COVERAGE_REQUEST_SEMANTICS)


class Fields:
    """A positional OCREQ field sequence that remembers where each named
    field starts within its section."""

    def __init__(self) -> None:
        self.data = bytearray()
        self.marks: dict[str, int] = {}

    def u32(self, value: int, name: str | None = None) -> "Fields":
        if name:
            self.marks[name] = len(self.data)
        self.data += struct.pack("<I", value)
        return self

    def raw(self, value: bytes, name: str | None = None) -> "Fields":
        if name:
            self.marks[name] = len(self.data)
        self.data += value
        return self

    def bytes(self, value: bytes, name: str | None = None) -> "Fields":
        return self.u32(len(value), name).raw(value)

    def replace_u32(self, name: str, value: int) -> "Fields":
        start = self.marks[name]
        self.data[start:start + 4] = struct.pack("<I", value)
        return self


def subject(snapshots: tuple = (), edges: tuple = (), root_role: int = 1) -> Fields:
    """One external-local package, its snapshot rows, the edge table and the
    root, in the subject table's order."""
    fields = Fields().u32(1, "packages")
    fields.bytes(b"a", "name").u32(1, "lineage_kind").bytes(b"l").bytes(b"r").bytes(b"t")
    fields.bytes(b"c").bytes(b"", "member").u32(1, "role").u32(len(snapshots), "snapshots")
    for index, row in enumerate(snapshots):
        kind = row[0]
        fields.u32(kind, f"snapshot{index}.kind").bytes(row[1])
        if kind == 2:
            fields.u32(row[2], f"snapshot{index}.executable").bytes(row[3])
        elif kind == 3:
            fields.bytes(row[2])
    fields.u32(len(edges), "edges")
    for index, (requester, scope, alias, target) in enumerate(edges):
        fields.u32(requester).u32(scope, f"edge{index}.scope")
        fields.bytes(alias).u32(target, f"edge{index}.target")
    return fields.u32(0, "root").u32(root_role, "root_role")


def invocation(product: int = 4, admissions: tuple = ()) -> Fields:
    fields = Fields().u32(product, "product").bytes(b"alpha_bootstrap")
    fields.u32(len(admissions), "admissions")
    for index, (kind, data) in enumerate(admissions):
        fields.u32(kind, f"admission{index}.kind").bytes(data)
    return fields.raw(b"\xaa" * 32, "commitment")


def request(subject_fields: Fields, invocation_fields: Fields) -> bytes:
    return (b"OCREQ\x01\x00\x00" + struct.pack("<II", len(subject_fields.data), len(invocation_fields.data))
            + bytes(subject_fields.data) + bytes(invocation_fields.data))


def check(program: str, name: str, payload: bytes, expected: bytes) -> None:
    CASES.append(name)
    completed = subprocess.run([program], input=payload, capture_output=True, check=False)
    assert completed.stdout == expected, f"{name}:\nexpected {expected.hex()}\nactual   {completed.stdout.hex()}"
    assert completed.returncode == expected[8], f"{name}: exit {completed.returncode}"


def shared_with_d(program: str) -> None:
    """D's request-gate vectors, byte for byte."""
    fixture = CANONICAL
    check(program, "d-canonical", fixture, SEMANTICS)
    check(program, "d-empty", b"", malformed(0))
    check(program, "d-corrupt-identity", b"N" + fixture[1:], malformed(0))
    check(program, "d-truncated-invocation", fixture[:100], malformed(12))
    check(program, "d-trailing-byte", fixture + b"\x00", malformed(132))
    check(program, "d-product-tag", fixture[:73] + b"\x05" + fixture[74:], malformed(73))
    # A byte field longer than its section is missing at the section end.
    check(program, "d-hostile-name-length", fixture[:20] + b"\xff\xff\xff\x7f" + fixture[24:], malformed(73))
    check(program, "d-zero-commitment", fixture[:-32] + b"\x00" * 32, SEMANTICS)
    check(program, "d-staging", b"A" * 65537,
          frame(INCOMPLETE, SPACE_REQUEST, REQUEST_STAGING_BYTES, 65536, 65536, 65537))


def envelope(program: str) -> None:
    """Phase 0: identity, header extent, high bits, extents, exact end. A
    stream shorter than a fixed field is missing its byte at the stream end."""
    check(program, "short-identity", CANONICAL[:5], malformed(5))
    check(program, "identity-version", b"OCREQ\x02" + CANONICAL[6:], malformed(5))
    check(program, "identity-reserved", CANONICAL[:7] + b"\x01" + CANONICAL[8:], malformed(7))
    check(program, "short-header", CANONICAL[:12], malformed(12))
    check(program, "subject-length-high-bit", CANONICAL[:11] + b"\x80" + CANONICAL[12:], malformed(8))
    check(program, "invocation-length-high-bit", CANONICAL[:15] + b"\x80" + CANONICAL[16:], malformed(12))
    check(program, "subject-past-end",
          CANONICAL[:8] + struct.pack("<I", len(CANONICAL)) + CANONICAL[12:], malformed(8))
    check(program, "invocation-past-end",
          CANONICAL[:12] + struct.pack("<I", len(CANONICAL)) + CANONICAL[16:], malformed(12))
    check(program, "declared-end-short", CANONICAL[:-1], malformed(12))


def structure(program: str) -> None:
    """Phases 2 and 6: field and tag shape, anchored at computed offsets."""
    assert request(subject(), invocation()) == CANONICAL, "the encoder no longer spells D's fixture"

    def subject_case(name: str, fields: Fields, anchor: str) -> None:
        check(program, name, request(fields, invocation()), malformed(HEADER + fields.marks[anchor]))

    # A snapshot table holding every row kind is well formed.
    rows = ((1, b"src"), (2, b"src/main.omg", 1, b"machine"), (3, b"link", b"src"))
    check(program, "snapshot-kinds", request(subject(rows), invocation()), SEMANTICS)
    subject_case("snapshot-kind-tag", subject(((4, b"x"),)), "snapshot0.kind")
    subject_case("snapshot-executable-tag", subject(((2, b"x", 2, b""),)), "snapshot0.executable")
    # A count that cannot fit at eight bytes a row refuses at the count.
    subject_case("snapshot-count-impossible", subject().replace_u32("snapshots", 1000), "snapshots")
    subject_case("package-lineage-tag", subject().replace_u32("lineage_kind", 0), "lineage_kind")

    # Edges are four words a row; the scope is a tag.
    check(program, "edge", request(subject(edges=((0, 2, b"alias", 0),)), invocation()), SEMANTICS)
    subject_case("edge-scope-tag", subject(edges=((0, 3, b"alias", 0),)), "edge0.scope")
    subject_case("edge-target-high-bit", subject(edges=((0, 1, b"", 0x80000000),)), "edge0.target")
    subject_case("root-role-tag", subject(root_role=3), "root_role")

    # Bytes past the subject's last field are trailing at the first one.
    trailing = subject().raw(b"\x00")
    check(program, "subject-trailing", request(trailing, invocation()), malformed(HEADER + len(trailing.data) - 1))
    # A byte field longer than its section is missing at the section end.
    long_member = subject().replace_u32("member", 4096)
    check(program, "member-past-section", request(long_member, invocation()),
          malformed(HEADER + len(long_member.data)))

    invocation_start = HEADER + len(subject().data)
    for product in (1, 2, 3):
        check(program, f"product-{product}", request(subject(), invocation(product)), SEMANTICS)
    check(program, "product-tag-zero", request(subject(), invocation(0)), malformed(invocation_start))
    # No admission kind is assigned yet, so a present row refuses at its kind.
    admitted = invocation(admissions=((1, b""),))
    check(program, "admission-unassigned-kind", request(subject(), admitted),
          malformed(invocation_start + admitted.marks["admission0.kind"]))
    short = invocation()
    short.data = short.data[:-1]
    check(program, "commitment-short", request(subject(), short), malformed(invocation_start + len(short.data)))
    long = invocation().raw(b"\x00")
    check(program, "invocation-trailing", request(subject(), long), malformed(invocation_start + len(long.data) - 1))


def main() -> None:
    program = sys.argv[1]
    shared_with_d(program)
    envelope(program)
    structure(program)
    print(f"Product request boundary: {len(CASES)} cases passed")


if __name__ == "__main__":
    main()
