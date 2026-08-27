#!/usr/bin/env python3
"""Responsibility-local controls for the modular OMGRFN21 family."""

from __future__ import annotations

import copy
import struct
import subprocess
import sys
from pathlib import Path

from omgrfn6_bundle import MAX_CKIR, MAX_FRAME, MAX_WITNESS
from omgrfn21_bundle import pack
from omgrfn21_ckir import check_selected_structure, decode, interpret, producer_decode
from omgrfn21_elf import reconstruct
from omgrfn21_frame import HEADER, split
from omgrfn21_profiles import CKIR_FIXTURE as cfix
from omgrfn21_profiles import SOURCE_FIXTURE as sfix
from omgrfn21_profiles import canonical, components
from omgrfn21_source import HEADER as WITNESS_HEADER
from omgrfn21_source import ROWS as WITNESS_ROWS
from omgrfn21_source import check_witness_relation, decode_witness

HERE = Path(__file__).resolve().parent
OWNERS = ("r1", "r2", "r3", "r4-lowering", "r4-source-result",
          "r5-structure", "r5-result", "r5-elf")


def u16(raw: bytes, at: int, value: int) -> bytes:
    changed = bytearray(raw); struct.pack_into("<H", changed, at, value); return bytes(changed)


def u32(raw: bytes, at: int, value: int) -> bytes:
    changed = bytearray(raw); struct.pack_into("<I", changed, at, value); return bytes(changed)


def witness_word(raw: bytes, table: str, row: int, word: int, value: int) -> bytes:
    parsed = decode_witness(raw)
    start = parsed.offsets[table][0] + row * WITNESS_ROWS[table].size
    return u32(raw, start + word * 4, value)


def observe(owner: str, raw: bytes, expected: int, label: str) -> None:
    result = subprocess.run(
        [sys.executable, "-B", str(HERE / f"omgrfn21-{owner}.py")],
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


def main() -> None:
    omgcomp, witness, ckir, elf = components()
    frame = canonical()
    parts = split(frame)
    base = cfix.tables()
    witness_at = HEADER.size + len(omgcomp)
    ckir_at = witness_at + len(witness)
    for owner in OWNERS:
        observe(owner, frame, 0, "canonical")

    # R1: exact outer/component identities, extents, EOF, result, and ceilings.
    for label, raw, status in (
        ("outer-magic", b"OMGRFNX\0" + frame[8:], 251),
        ("outer-version", u32(frame, 8, 20), 251),
        ("outer-eof", frame + b"x", 251),
        ("component-extent", u32(frame, 16, len(omgcomp) + 1), 251),
        ("witness-major", u16(frame, witness_at + 8, 9), 251),
        ("ckir-major", u16(frame, ckir_at + 8, 16), 251),
        ("result", HEADER.pack(b"OMGRFNL\0", 21, 1, len(omgcomp), len(witness),
                               len(ckir), len(elf), 71, 71)
                   + omgcomp + witness + ckir + elf, 251),
    ):
        observe("r1", raw, status, label)
    observe("r1", u32(frame, 20, MAX_WITNESS + 1), 252, "witness-resource")
    observe("r1", u32(frame, 24, MAX_CKIR + 1), 252, "ckir-resource")
    observe("r1", frame + bytes(MAX_FRAME + 1 - len(frame)), 252,
            "whole-frame-resource")

    # R2: policy distinction, semantic rows/spans, pairing, and N=65537.
    index_flag_drift = bytearray(witness)
    type_start = decode_witness(witness).offsets["types"][0]
    index_flag_drift[type_start + 3 * WITNESS_ROWS["types"].size + 5] = 0
    observe("r2", pack(omgcomp, bytes(index_flag_drift), ckir, elf), 251,
            "lookup-index-policy")
    length_flag_drift = bytearray(witness)
    length_flag_drift[type_start + 4 * WITNESS_ROWS["types"].size + 5] = 1
    observe("r2", pack(omgcomp, bytes(length_flag_drift), ckir, elf), 251,
            "retained-length-policy")
    changed_source = sfix.CANONICAL.replace("self.length + 1", "1 + self.length")
    changed_envelope = sfix.encode_compilation(changed_source)
    observe("r2", pack(changed_envelope, witness, ckir, elf), 251,
            "valid-envelope-cross-pair")
    unqualified_index = sfix.CANONICAL.replace(
        "index: u64 in Trapping", "index: u64            ")
    observe("r2", pack(sfix.encode_compilation(unqualified_index), witness, ckir, elf),
            251, "source-policy-cannot-be-fabricated-by-witness")
    parsed_witness = decode_witness(witness)
    declaration_call = witness_word(
        witness, "calls", 0, 5,
        parsed_witness.tables["machines"][parsed_witness.clear_machine][9])
    observe("r2", pack(omgcomp, declaration_call, ckir, elf), 251,
            "call-span-cannot-select-machine-declaration")
    n_over = u32(witness, 76, 65_537)
    observe("r2", pack(omgcomp, n_over, ckir, elf), 251, "array-65537-malformed")

    # R3: complete selected CKIR structure, artifact-local resources and EOF.
    index_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 4)
    add_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 8)
    less_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 9)
    for label, raw in (
        ("index-immediate", changed_ckir(base, "operations", index_op, 10, 1)),
        ("add-immediate", changed_ckir(base, "operations", add_op, 11, 1)),
        ("missing-less", changed_ckir(base, "operations", less_op, 3, 12)),
    ):
        # R3 intentionally does not read the artifact component.
        observe("r3", pack(omgcomp, witness, raw, elf), 251, label)
    operations_over = cfix.mutate_count(ckir, "operations", 32_769)
    observe("r3", pack(omgcomp, witness, operations_over, elf), 252,
            "operation-resource")
    observe("r5-structure", pack(omgcomp, witness, operations_over, elf), 252,
            "independent-operation-resource")
    observe("r5-structure", pack(omgcomp, witness, ckir + b"x", elf), 251,
            "ckir-exact-eof")

    # R4: R3 accepts commutative Add order, but authored source order is exact.
    reordered = copy.deepcopy(base)
    start = reordered["operations"][add_op][8]
    reordered["operands"][start], reordered["operands"][start + 1] = \
        reordered["operands"][start + 1], reordered["operands"][start]
    reordered_ckir = cfix.encode(reordered)
    check_selected_structure(producer_decode(reordered_ckir))
    observe("r4-lowering", pack(omgcomp, witness, reordered_ckir,
                                reconstruct(reordered_ckir)), 251,
            "leaf-plus-literal-order")

    # R5: exact abstract result, runtime teeth, and independent artifact bytes.
    byte70 = next(i for i, row in enumerate(base["operations"])
                  if row[1] == 3 and row[3] == 1 and row[10] == 70)
    result_drift = changed_ckir(base, "operations", byte70, 10, 69)
    require_result = interpret(decode(result_drift))
    if require_result == 70:
        raise RuntimeError("result mutation did not change CKIR meaning")
    observe("r5-result", pack(omgcomp, witness, result_drift,
                              reconstruct(result_drift)), 251, "result-drift")
    observe("r5-elf", pack(omgcomp, witness, ckir,
                           elf[:-1] + bytes([elf[-1] ^ 1])), 251,
            "artifact-byte-drift")
    for extra, label in (("carry", "u64-carry"),
                         ("interval", "destination-interval"),
                         ("index-oob-high", "high-half-index")):
        runtime = cfix.encode(cfix.tables(extra=extra))
        decode(runtime)
        try:
            interpret(decode(runtime))
        except ValueError:
            pass
        else:
            raise RuntimeError(f"{label} runtime trap was not observed")

    image = reconstruct(ckir)
    for needle, label in ((b"\x49\xb9", "imm64 index bound"),
                          (b"\x0f\x83", "unsigned index JAE"),
                          (b"\x48\x03\x85", "qword Add"),
                          (b"\x0f\x82", "unsigned carry trap"),
                          (b"\x48\x3b\x85", "qword Less")):
        if needle not in image:
            raise RuntimeError(f"missing reconstructed {label}")
    print("OMGRFN21 modular owners: positive, local/cross/resource/artifact controls PASS")


if __name__ == "__main__":
    main()
