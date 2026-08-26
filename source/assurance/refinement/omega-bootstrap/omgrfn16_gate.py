#!/usr/bin/env python3
"""Untrusted producer/profile plumbing for the OMGRFN16 same-frame gate."""

from __future__ import annotations

import argparse
import importlib.util
import struct
import subprocess
import sys
from pathlib import Path

from omgrfn16_frame import HEADER, NO_RESULT


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

    witness = bytearray(add_parts[1]); needle = struct.pack("<BBHIIII", 2, 1, 0, 0, 0, 0, NO_RESULT)
    at = witness.find(needle)
    if at < 0: raise ValueError("semantic high-word mutation anchor")
    struct.pack_into("<I", witness, at + 16, 0x7FFF_FFFF)
    write("witness-high-word", pack(add_parts[0], witness, add_parts[2], add_parts[3]))

    elf = bytearray(add_parts[3]); elf[4096] ^= 1
    write("elf-instruction", pack(add_parts[0], add_parts[1], add_parts[2], elf))
    write("elf-trailing", pack(add_parts[0], add_parts[1], add_parts[2], add_parts[3] + b"\0"))


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
