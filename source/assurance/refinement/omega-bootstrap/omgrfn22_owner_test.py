#!/usr/bin/env python3
"""Responsibility-local controls for the modular OMGRFN22 family."""

from __future__ import annotations

import copy
import struct
import subprocess
import sys
from pathlib import Path

from omgrfn6_bundle import MAX_CKIR, MAX_FRAME, MAX_WITNESS
from omgrfn22_bundle import pack
from omgrfn22_ckir import IR19, arguments, producer_decode
from omgrfn22_elf import reconstruct
from omgrfn22_frame import HEADER, split
from omgrfn22_profiles import CKIR_FIXTURE as cfix
from omgrfn22_profiles import SOURCE_FIXTURE as sfix
from omgrfn22_profiles import canonical, components
from omgrfn22_source import ROWS as WROWS
from omgrfn22_source import decode_witness

HERE = Path(__file__).resolve().parent
OWNERS = ("r1", "r2", "r3", "r4-lowering", "r4-source-result",
          "r5-structure", "r5-result", "r5-elf")


def u16(raw: bytes, at: int, value: int) -> bytes:
    changed = bytearray(raw); struct.pack_into("<H", changed, at, value); return bytes(changed)


def u32(raw: bytes, at: int, value: int) -> bytes:
    changed = bytearray(raw); struct.pack_into("<I", changed, at, value); return bytes(changed)


def witness_word(raw: bytes, table: str, row: int, word: int, value: int) -> bytes:
    parsed = decode_witness(raw)
    start = parsed.offsets[table][0] + row * WROWS[table].size
    return u32(raw, start + word * 4, value)


def observe(owner: str, raw: bytes, expected: int, label: str) -> None:
    result = subprocess.run(
        [sys.executable, "-B", str(HERE / f"omgrfn22-{owner}.py")],
        input=raw, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=8,
    )
    if result.returncode != expected:
        raise RuntimeError(f"{label}/{owner}: {result.returncode} != {expected}: "
                           f"{result.stderr.decode('utf-8', 'replace')}")
    if result.stdout:
        raise RuntimeError(f"{label}/{owner}: published stdout")


def changed_ckir(base, table: str, row: int, field: int, value: int) -> bytes:
    tables = copy.deepcopy(base)
    tables[table][row] = cfix.replace(tables[table][row], field, value)
    return cfix.encode(tables)


def meaning(contents: bytes) -> int:
    return IR19.interpret(IR19.decode(contents))


def main() -> None:
    omgcomp, witness, ckir, elf = components()
    frame = canonical()
    split(frame)
    base = cfix.tables()
    witness_at = HEADER.size + len(omgcomp)
    ckir_at = witness_at + len(witness)
    for owner in OWNERS:
        observe(owner, frame, 0, "canonical")

    # R1: exact outer/component identities, extents, EOF, result, and ceilings.
    for label, raw, status in (
        ("outer-magic", b"OMGRFNX\0" + frame[8:], 251),
        ("outer-version", u16(frame, 8, 21), 251),
        ("outer-eof", frame + b"x", 251),
        ("component-extent", u32(frame, 16, len(omgcomp) + 1), 251),
        ("witness-major", u16(frame, witness_at + 8, 10), 251),
        ("ckir-major", u16(frame, ckir_at + 8, 18), 251),
        ("result", HEADER.pack(b"OMGRFNM\0", 22, 1, len(omgcomp), len(witness),
                               len(ckir), len(elf), 71, 71)
                   + omgcomp + witness + ckir + elf, 251),
    ):
        observe("r1", raw, status, label)
    observe("r1", u32(frame, 20, MAX_WITNESS + 1), 252, "witness-resource")
    observe("r1", u32(frame, 24, MAX_CKIR + 1), 252, "ckir-resource")
    observe("r1", frame + bytes(MAX_FRAME + 1 - len(frame)), 252,
            "whole-frame-resource")

    # R2: authored copy/policy, owner/ordinal/type, spans, guards, and pairing.
    type_at = decode_witness(witness).offsets["types"][0]
    index_flag = bytearray(witness)
    index_flag[type_at + 4 * WROWS["types"].size + 5] = 0
    observe("r2", pack(omgcomp, bytes(index_flag), ckir, elf), 251,
            "authored-u64-policy")
    observe("r2", pack(omgcomp, witness_word(witness, "types", 4, 8,
            0xFFFF_FFFE), ckir, elf), 251, "full-u64-upper-low-word")
    for label, table, row, word, value in (
        ("observation-copy", "records", 0, 7, 0),
        ("field-owner", "fields", 4, 1, 1),
        ("field-ordinal", "fields", 4, 2, 5),
        ("field-type", "fields", 4, 3, 1),
        ("store-path", "stores", 8, 5, 7),
    ):
        observe("r2", pack(omgcomp, witness_word(witness, table, row, word, value),
                           ckir, elf), 251, label)
    changed_source = sfix.CANONICAL.replace("self.count + 1", "1 + self.count")
    observe("r2", pack(sfix.encode(changed_source), witness, ckir, elf), 251,
            "valid-envelope-exact-increment-cross-pair")
    changed_guard = sfix.CANONICAL.replace("self.count < 16384",
                                           "self.count < 16383")
    observe("r2", pack(sfix.encode(changed_guard), witness, ckir, elf), 251,
            "valid-envelope-guard-cross-pair")
    unqualified_u32 = sfix.CANONICAL.replace("source: u32 in Trapping",
                                             "source: u32            ")
    observe("r2", pack(sfix.encode(unqualified_u32), witness, ckir, elf), 251,
            "source-policy-cannot-be-fabricated")

    # R3: complete CKIR record shape, nested places, access, widths, and resources.
    index_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 4)
    nested_op = next(i for i, row in enumerate(base["operations"])
                     if row[3] == 3 and row[10] == 4 and i > index_op)
    store_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 6
                    and row[2] == 1)
    load_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 5
                   and row[1] == 1 and row[2] == 4)
    for label, raw in (
        ("record-copy", changed_ckir(base, "records", 0, 4, 0)),
        ("record-owner", changed_ckir(base, "fields", 4, 1, 1)),
        ("record-ordinal", changed_ckir(base, "fields", 4, 2, 5)),
        ("record-width", changed_ckir(base, "fields", 4, 3, 0)),
        ("nested-field-path", changed_ckir(base, "operations", nested_op, 10, 5)),
        ("index-immediate", changed_ckir(base, "operations", index_op, 10, 1)),
        ("store-not-store", changed_ckir(base, "operations", store_op, 3, 5)),
        ("load-not-load", changed_ckir(base, "operations", load_op, 3, 6)),
        ("mutable-block-lost", changed_ckir(base, "blocks", 1, 2, 1)),
    ):
        observe("r3", pack(omgcomp, witness, raw, elf), 251, label)
    adjacent = cfix.encode(cfix.tables(array_length=52_428))
    over = cfix.encode(cfix.tables(array_length=52_429))
    observe("r3", pack(omgcomp, witness, adjacent, elf), 251,
            "below-2m-but-outside-selected-N")
    observe("r3", pack(omgcomp, witness, over, elf), 252, "owner-over-2m")
    operations_over = cfix.mutate_count(ckir, "operations", 32_769)
    observe("r3", pack(omgcomp, witness, operations_over, elf), 252,
            "operation-resource")
    observe("r5-structure", pack(omgcomp, witness, ckir + b"x", elf), 251,
            "ckir-exact-eof")

    # R4: a valid high-half transport profile is not the authored source pair.
    high_transport = cfix.encode(cfix.tables(extra="high-half-transport"))
    producer_decode(high_transport)
    if meaning(high_transport) != 70:
        raise RuntimeError("high-half transport positive changed result")
    observe("r4-lowering", pack(omgcomp, witness, high_transport,
                                reconstruct(high_transport)), 251,
            "structurally-valid-CKIR-cross-pair")

    # R5: boundary observations and every conservative artifact byte.
    bound = cfix.tables(extra="index-oob-bound")
    bound_index = [row for row in bound["operations"] if row[3] == 4][-1]
    bound_value = arguments(type("M", (), {"tables": bound})(), bound_index)[1]
    bound_const = next(i for i, row in enumerate(bound["operations"])
                       if row[4] == 1 and row[6] == bound_value)
    bound["operations"][bound_const] = cfix.replace(
        cfix.replace(bound["operations"][bound_const], 7, 3), 10, 16_383)
    if meaning(cfix.encode(bound)) != 70:
        raise RuntimeError("N-1 IndexPlace should remain admitted")
    for extra, label in (("index-oob-bound", "index-N"),
                         ("index-oob-high", "high-half-index")):
        runtime = cfix.encode(cfix.tables(extra=extra))
        try:
            meaning(runtime)
        except ValueError:
            pass
        else:
            raise RuntimeError(f"{label} runtime trap was not observed")

    for needle, label in (
        (b"\x48\x69\xc0\x28\x00\x00\x00", "record stride 40"),
        (b"\x0f\x80", "stride overflow JO"),
        (b"\x49\x01\xc2", "record address addition"),
        (b"\x0f\x82", "address/carry JB"),
        (b"\x48\x05\x20\x00\x00\x00", "field offset 32"),
    ):
        if needle not in elf:
            raise RuntimeError(f"missing reconstructed {label}")
    for needle, label in (
        (b"\x48\x69\xc0\x28\x00\x00\x00", "artifact-stride"),
        (b"\x48\x05\x20\x00\x00\x00", "artifact-field-offset"),
    ):
        changed = bytearray(elf); at = changed.index(needle); changed[at + len(needle) - 1] ^= 1
        observe("r5-elf", pack(omgcomp, witness, ckir, bytes(changed)), 251, label)
    observe("r5-elf", pack(omgcomp, witness, ckir, elf[:-1] + b"x"), 251,
            "artifact-byte-drift")
    print("OMGRFN22 modular owners: positive, local/cross/resource/artifact controls PASS")


if __name__ == "__main__":
    main()
