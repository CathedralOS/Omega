#!/usr/bin/env python3
"""Responsibility-local controls for the modular OMGRFN23 family."""

from __future__ import annotations

import copy
import struct
import subprocess
import sys
from pathlib import Path

from omgrfn6_bundle import MAX_CKIR, MAX_FRAME, MAX_WITNESS
from omgrfn23_bundle import pack
from omgrfn23_ckir import IR20, arguments, producer_decode
from omgrfn23_elf import reconstruct
from omgrfn23_frame import HEADER, FLAG_PROPOSITION, MAGIC, VERSION, split
from omgrfn23_profiles import CKIR_FIXTURE as cfix
from omgrfn23_profiles import SOURCE_FIXTURE as sfix
from omgrfn23_profiles import canonical, components
from omgrfn23_source import ROWS as WROWS
from omgrfn23_source import decode_witness

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
        [sys.executable, "-B", str(HERE / f"omgrfn23-{owner}.py")],
        input=raw, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10,
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


def unchecked_frame(omgcomp: bytes, witness: bytes, ckir: bytes, elf: bytes) -> bytes:
    return HEADER.pack(MAGIC, VERSION, FLAG_PROPOSITION, len(omgcomp),
                       len(witness), len(ckir), len(elf), 70, 70) + \
        omgcomp + witness + ckir + elf


def main() -> None:
    omgcomp, witness, ckir, elf = components()
    frame = canonical(); split(frame)
    for owner in OWNERS:
        observe(owner, frame, 0, "canonical")

    witness_at = HEADER.size + len(omgcomp)
    ckir_at = witness_at + len(witness)
    for label, raw, code in (
        ("outer-magic", b"OMGRFNX\0" + frame[8:], 251),
        ("outer-version", u16(frame, 8, 22), 251),
        ("outer-eof", frame + b"x", 251),
        ("component-extent", u32(frame, 16, len(omgcomp) + 1), 251),
        ("witness-major", u16(frame, witness_at + 8, 11), 251),
        ("ckir-major", u16(frame, ckir_at + 8, 19), 251),
        ("result", HEADER.pack(MAGIC, VERSION, FLAG_PROPOSITION,
                               len(omgcomp), len(witness), len(ckir), len(elf),
                               71, 71) + omgcomp + witness + ckir + elf, 251),
    ):
        observe("r1", raw, code, label)
    observe("r1", u32(frame, 20, MAX_WITNESS + 1), 252, "witness-resource")
    observe("r1", u32(frame, 24, MAX_CKIR + 1), 252, "ckir-resource")
    observe("r1", frame + bytes(MAX_FRAME + 1 - len(frame)), 252,
            "whole-frame-resource")

    # R2 owns source pairing, copy declarations, policies, paths, arguments,
    # guard/increment, Float readback, result, and local table resources.
    for label, table, row, word, value in (
        ("record-copy", "records", 3, 7, 0),
        ("sum-copy", "sums", 3, 7, 0),
        ("field-owner", "fields", 15, 1, 4),
        ("field-type", "fields", 15, 3, 13),
        ("store-path", "store_paths", 2, 0, 4),
        ("source-projection", "stores", 10, 8, 0),
        ("call-arity", "calls", 0, 8, 9),
        ("float-constructor", "arguments", 1, 5, 77),
    ):
        observe("r2", pack(omgcomp, witness_word(witness, table, row, word, value),
                           ckir, elf), 251, label)
    observe("r2", pack(omgcomp, u32(witness, 28, 2049), ckir, elf), 252,
            "source-type-resource")
    for label, source in (
        ("guard-cross-pair", sfix.CANONICAL.replace(
            "self.token_count < 16384", "self.token_count < 16383", 1)),
        ("increment-cross-pair", sfix.CANONICAL.replace(
            "self.token_count + 1", "self.token_count + 2")),
        ("float-cross-pair", sfix.CANONICAL.replace(
            "has_exponent: true, empty_exponent: false, has_suffix: true",
            "has_exponent: true, empty_exponent: true, has_suffix: true", 1)),
    ):
        observe("r2", pack(sfix.encode(source), witness, ckir, elf), 251, label)

    # R3 owns all generic and selected CKIR20 malformed/resource behavior.
    for label, (bad, code) in cfix.malformed().items():
        observe("r3", unchecked_frame(omgcomp, witness, bad, elf), code, label)
    observe("r5-structure", pack(omgcomp, witness, ckir + b"x", elf), 251,
            "CKIR exact EOF")

    high = cfix.encode(cfix.tables(extra="high-half-transport"))
    producer_decode(high)
    if IR20.interpret(IR20.decode(high)) != 70:
        raise RuntimeError("high-half transport positive changed result")
    observe("r4-lowering", pack(omgcomp, witness, high, reconstruct(high)), 251,
            "structurally valid CKIR cross-pair")

    # R5 meaning boundaries plus exact artifact templates and arbitrary drift.
    for extra in ("index-oob-bound", "index-oob-high"):
        runtime = cfix.encode(cfix.tables(extra=extra))
        try:
            IR20.interpret(IR20.decode(runtime))
        except ValueError:
            pass
        else:
            raise RuntimeError(f"{extra} runtime trap was not observed")
    for needle, label in (
        (b"\x48\x69\xc0\x38\x00\x00\x00", "Token stride56"),
        (b"\x48\x69\xc0\x28\x00\x00\x00", "Observation stride40"),
        (b"\x41\x8b\x0b\x89\xca\x89\xc8", "semantic sum Copy"),
        (b"\x41\x8b\x03\x3d\x09\x00\x00\x00", "dispatch tag guard"),
        (b"\x41\xc7\x02\x02\x00\x00\x00", "Float constructor"),
    ):
        if needle not in elf:
            raise RuntimeError(f"missing reconstructed {label}")
        changed = bytearray(elf); at = changed.index(needle); changed[at] ^= 1
        observe("r5-elf", pack(omgcomp, witness, ckir, bytes(changed)), 251,
                f"artifact-{label}")
    observe("r5-elf", pack(omgcomp, witness, ckir, elf[:-1] + b"x"), 251,
            "artifact arbitrary-byte drift")
    print("OMGRFN23 modular owners: positive, local/cross/resource/artifact controls PASS")


if __name__ == "__main__":
    main()
