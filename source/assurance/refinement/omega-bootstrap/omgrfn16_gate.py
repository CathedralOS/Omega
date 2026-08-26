#!/usr/bin/env python3
"""Untrusted producer/profile plumbing for the OMGRFN16 same-frame gate."""

from __future__ import annotations

import argparse
import importlib.util
import struct
import subprocess
import sys
from pathlib import Path

from omgrfn16_frame import (
    HEADER,
    MAX_CKIR,
    MAX_ELF,
    MAX_FRAME,
    MAX_OMGCOMP,
    MAX_WITNESS,
    NO_RESULT,
)


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "bootstrap/omega-bootstrap/gates"
CASES = GATES / "fixtures/ckir14-arithmetic-cases"
sys.path.insert(0, str(GATES))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec); sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


PRODUCER = load("omgrfn16_producer_gate", GATES / "delta-resolved-to-ckir14-fixture.py")
IR14 = PRODUCER.ir14


def boundary_source(value: int) -> str:
    return f"""data Boundary {{ result: u32 in Trapping; }}
machine Boundary::run(&mut self) -> u8 {{
    self.result = {value} - 0;
    transition self.result == {value} {{ true -> passed() false -> failed() }}
    state passed(&mut self) {{ 70 }}
    state failed(&mut self) {{ 0 }}
}}
"""


def profiles() -> dict[str, tuple[str, bool]]:
    fixture = lambda name: (CASES / f"{name}.omg").read_text(encoding="ascii")
    return {
        "add-success": (fixture("add-only-selected"), False),
        "mixed-success": (fixture("precedence-association-parentheses"), False),
        "contexts-success": (fixture("representative-contexts"), False),
        "depth-eight-success": (fixture("depth-8-boundary"), False),
        "signed-boundary-success": (boundary_source(0x8000_0000), False),
        "upper-neighbor-success": (boundary_source(0xFFFF_FFFE), False),
        "maximum-success": (PRODUCER.FULL_LITERAL, False),
        "widen-success": (PRODUCER.WIDEN_ARITHMETIC, False),
        "view-composition-success": (fixture("ckir12-view-plus-arithmetic"), False),
        "add-overflow": (fixture("add-overflow"), True),
        "subtract-underflow": (fixture("nested-underflow"), True),
        "multiply-overflow": (fixture("multiply-overflow"), True),
    }


def run(executable: Path, contents: bytes, label: str) -> bytes:
    result = subprocess.run([str(executable)], input=contents, stdout=subprocess.PIPE)
    if result.returncode != 0 or not result.stdout:
        raise ValueError(f"{label} returned {result.returncode} without a carrier")
    return result.stdout


def pack(omgcomp: bytes, witness: bytes, ckir: bytes, elf: bytes,
         *, trap: bool = False, result: int = 70,
         magic: bytes = b"OMGRFNG\0", version: int = 16,
         flags: int | None = None) -> bytes:
    selected_flags = (3 if trap else 1) if flags is None else flags
    selected_result = NO_RESULT if trap else result
    selected_exit = NO_RESULT if trap else result & 255
    return HEADER.pack(
        magic, version, selected_flags, len(omgcomp), len(witness), len(ckir),
        len(elf), selected_result, selected_exit,
    ) + omgcomp + witness + ckir + elf


def produce(resolver: Path, lowerer: Path, backend: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    for name, (source, trap) in profiles().items():
        omgcomp = PRODUCER.encode_source(source)
        witness = run(resolver, omgcomp, f"{name} resolver")
        if witness[:12] != b"OMGRSW7\0\x07\0\0\0":
            raise ValueError(f"{name} did not select exact OMGRSW7")
        ckir = run(lowerer, PRODUCER.pack_lowering(omgcomp, witness), f"{name} lowerer")
        IR14.decode(ckir)
        elf = run(backend, ckir, f"{name} backend")
        components = {"omgc": omgcomp, "witness": witness, "ckir": ckir, "elf": elf}
        for suffix, contents in components.items():
            (output / f"{name}.{suffix}").write_bytes(contents)
        (output / f"{name}.rfn").write_bytes(pack(omgcomp, witness, ckir, elf, trap=trap))
    (output / "profiles.tsv").write_text("".join(
        f"{name}\t{'trap' if trap else 'result'}\n"
        for name, (_, trap) in profiles().items()
    ), encoding="ascii")


def components(frame: bytes) -> tuple[bytes, bytes, bytes, bytes]:
    fields = HEADER.unpack_from(frame)
    at = HEADER.size; result = []
    for length in fields[3:7]:
        result.append(frame[at:at + length]); at += length
    return tuple(result)  # type: ignore[return-value]


def controls(directory: Path) -> None:
    add = (directory / "add-success.rfn").read_bytes()
    mixed = (directory / "mixed-success.rfn").read_bytes()
    trap = (directory / "add-overflow.rfn").read_bytes()
    add_parts, mixed_parts = components(add), components(mixed)
    depth_parts = components((directory / "depth-eight-success.rfn").read_bytes())
    view_parts = components((directory / "view-composition-success.rfn").read_bytes())
    context_parts = components((directory / "contexts-success.rfn").read_bytes())

    def write(name: str, contents: bytes) -> None:
        (directory / f"control-{name}.rfn").write_bytes(contents)

    changed = bytearray(add); changed[6] = ord("F"); struct.pack_into("<I", changed, 8, 15)
    write("retired-outer15", changed)
    for name, flags in (("flags0", 0), ("flags2", 2), ("unknown-flags", 5)):
        changed = bytearray(add); struct.pack_into("<I", changed, 12, flags); write(name, changed)
    changed = bytearray(add); struct.pack_into("<II", changed, 32, NO_RESULT, 255)
    write("u32-max-success-framing", changed)

    witness = bytearray(add_parts[1]); witness[7] = 0; witness[:8] = b"OMGRSW6\0"
    write("retired-witness6", pack(add_parts[0], witness, add_parts[2], add_parts[3]))
    ckir = bytearray(add_parts[2]); struct.pack_into("<H", ckir, 8, 13)
    write("retired-ckir13", pack(add_parts[0], add_parts[1], ckir, add_parts[3]))

    changed = bytearray(add); struct.pack_into("<II", changed, 32, 71, 71)
    write("claim71", changed)
    write("trap-as-result", pack(*components(trap), result=0))
    write("source-ckir-cross", pack(add_parts[0], add_parts[1], mixed_parts[2], mixed_parts[3]))
    write("ckir-elf-cross", pack(add_parts[0], add_parts[1], add_parts[2], mixed_parts[3]))

    source = bytearray(add_parts[0]); marker = b"self.a + self.b"; at = source.find(marker)
    if at < 0: raise ValueError("operator mutation anchor")
    source[at + len(b"self.a ")] = ord("-")
    write("source-operator", pack(source, add_parts[1], add_parts[2], add_parts[3]))

    add_source = (CASES / "add-only-selected.omg").read_text(encoding="ascii")
    # Keep the source extent and every later block/name span unchanged so this
    # control isolates a valid same-carrier named-leaf substitution. R2 should
    # accept its still-canonical OMGRSW7; only the source/lowering and result
    # owners reject the stale CKIR/claim pairing.
    leaf_source = add_source.replace("self.a + self.b", "self.b + self.b", 1)
    if leaf_source == add_source: raise ValueError("leaf-name mutation anchor")
    write("source-leaf-name", pack(
        PRODUCER.encode_source(leaf_source), add_parts[1], add_parts[2], add_parts[3]
    ))
    grown_source = add_source.replace("self.a + self.b", "self.a + self.b ", 1)
    if grown_source == add_source: raise ValueError("growing-source mutation anchor")
    write("source-grown-stale-witness", pack(
        PRODUCER.encode_source(grown_source), add_parts[1], add_parts[2], add_parts[3]
    ))

    depth9 = (CASES / "depth-9-exhausted.omg").read_text(encoding="ascii")
    write("source-depth-nine", pack(
        PRODUCER.encode_source(depth9), depth_parts[1], depth_parts[2], depth_parts[3]
    ))

    view_source = (CASES / "ckir12-view-plus-arithmetic.omg").read_text(encoding="ascii")
    changed_view = view_source.replace('"F"', '"G"', 1)
    if changed_view == view_source: raise ValueError("view-literal mutation anchor")
    write("source-view-literal", pack(
        PRODUCER.encode_source(changed_view), view_parts[1], view_parts[2], view_parts[3]
    ))

    context_source = (CASES / "representative-contexts.omg").read_text(encoding="ascii")
    transition = "receive(prefix, (self.byte0 - 192) * 64 + (self.byte1 - 128), 9)"
    changed_transition = context_source.replace(transition, transition[:-2] + "8)", 1)
    if changed_transition == context_source: raise ValueError("transition sibling mutation anchor")
    write("source-transition-sibling", pack(
        PRODUCER.encode_source(changed_transition), context_parts[1], context_parts[2], context_parts[3]
    ))

    envelope = bytearray(add_parts[0]); envelope[0] ^= 1
    write("malformed-omgcomp", pack(envelope, add_parts[1], add_parts[2], add_parts[3]))

    for name, offset, ceiling in (
        ("omgcomp-resource", 16, MAX_OMGCOMP),
        ("witness-resource", 20, MAX_WITNESS),
        ("ckir-resource", 24, MAX_CKIR),
        ("elf-resource", 28, MAX_ELF),
    ):
        changed = bytearray(add); struct.pack_into("<I", changed, offset, ceiling + 1)
        write(name, changed)
    write("whole-frame-resource", add + b"\0" * (MAX_FRAME + 1 - len(add)))

    witness = bytearray(add_parts[1]); needle = struct.pack("<BBHIIII", 2, 1, 0, 0, 0, 0, NO_RESULT)
    at = witness.find(needle)
    if at < 0: raise ValueError("semantic high-word mutation anchor")
    struct.pack_into("<I", witness, at + 16, 0x7FFF_FFFF)
    write("witness-high-word", pack(add_parts[0], witness, add_parts[2], add_parts[3]))

    elf = bytearray(add_parts[3]); elf[4096] ^= 1
    write("elf-instruction", pack(add_parts[0], add_parts[1], add_parts[2], elf))
    write("elf-trailing", pack(add_parts[0], add_parts[1], add_parts[2], add_parts[3] + b"\0"))

    for name, marker, delta in (
        ("elf-case-tag", b"\x41\xc7\x02\x00\x00\x00\x00", 3),
        ("elf-dispatch-bound", b"\x41\x8b\x03\x3d\x02\x00\x00\x00", 4),
    ):
        elf = bytearray(context_parts[3]); at = elf.find(marker)
        if at < 0: raise ValueError(f"{name} mutation anchor")
        elf[at + delta] ^= 1
        write(name, pack(context_parts[0], context_parts[1], context_parts[2], elf))


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    item = sub.add_parser("produce")
    item.add_argument("resolver", type=Path); item.add_argument("lowerer", type=Path)
    item.add_argument("backend", type=Path); item.add_argument("output", type=Path)
    item = sub.add_parser("controls"); item.add_argument("directory", type=Path)
    args = parser.parse_args()
    if args.command == "produce": produce(args.resolver, args.lowerer, args.backend, args.output)
    else: controls(args.directory)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, struct.error) as error:
        raise SystemExit(f"OMGRFN16 gate plumbing: {error}")
