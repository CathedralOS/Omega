#!/usr/bin/env python3
"""Independently decode and account for the admitted Beta compiler subject."""

from collections import Counter
import hashlib
from pathlib import Path

import beta_ref


ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / "bootstrap/beta/compiler/beta_compiler.beta"
TAPE = ROOT / "bootstrap/beta/compiler/beta_compiler_bytecode.tape"

SOURCE_SIZE = 12_536
SOURCE_SHA256 = "2f9a9f55a2c708731567367380521bfe35b1c13a00a367febd1f9f654c25f320"
TAPE_SIZE = 1_773
TAPE_SHA256 = "4b1572a5ce406fc5b194e047ac6c5c601c1860261933381443a08c9041b77361"
CODE_END = 0x64D

# This table is copied from Alpha's written encoding, not from the compiler's
# mnemonic table or the independent Beta assembler.
ALPHA = {
    0x00: ("halt", "r"), 0x01: ("imm", "rx"),
    0x02: ("mov", "rr"), 0x03: ("add", "rr"),
    0x04: ("sub", "rr"), 0x05: ("mul", "rr"),
    0x06: ("div", "rr"), 0x07: ("mod", "rr"),
    0x08: ("loadb", "rr"), 0x09: ("storeb", "rr"),
    0x0A: ("load", "rr"), 0x0B: ("store", "rr"),
    0x0C: ("jmp", "x"), 0x0D: ("jz", "rx"),
    0x0E: ("jnz", "rx"), 0x0F: ("jlt", "rrx"),
    0x10: ("jeq", "rrx"), 0x11: ("read", "r"),
    0x12: ("write", "r"), 0x13: ("call", "x"),
    0x14: ("ret", ""),
}

EXPECTED_INVENTORY = {
    "add": 24, "call": 12, "halt": 4, "imm": 37, "jeq": 19,
    "jlt": 24, "jmp": 26, "jnz": 3, "jz": 5, "load": 1,
    "loadb": 16, "mov": 55, "mul": 1, "read": 2, "ret": 12,
    "store": 1, "storeb": 1, "sub": 7, "write": 3,
}

EXPECTED_TABLE = [
    ("halt", "1"), ("imm", "18"), ("mov", "11"),
    ("add", "11"), ("sub", "11"), ("mul", "11"),
    ("div", "11"), ("mod", "11"), ("loadb", "11"),
    ("storeb", "11"), ("load", "11"), ("store", "11"),
    ("jmp", "8"), ("jz", "18"), ("jnz", "18"),
    ("jlt", "118"), ("jeq", "118"), ("read", "1"),
    ("write", "1"), ("call", "8"), ("ret", ""), ("dw", ""),
]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"Beta root audit: {message}")


def decode(tape: bytes) -> dict[int, tuple[str, tuple[int, ...], int]]:
    instructions = {}
    cursor = 0
    while cursor < CODE_END:
        start = cursor
        opcode = tape[cursor]
        cursor += 1
        require(opcode in ALPHA, f"unknown Alpha opcode {opcode:#x} at {start:#x}")
        name, kinds = ALPHA[opcode]
        operands = []
        for kind in kinds:
            width = 1 if kind == "r" else 8
            require(cursor + width <= CODE_END, f"truncated {name} at {start:#x}")
            operands.append(int.from_bytes(tape[cursor:cursor + width], "little"))
            cursor += width
        instructions[start] = (name, tuple(operands), cursor)
    require(cursor == CODE_END, "instruction partition does not end at mnemonic table")
    return instructions


def table_rows(data: bytes) -> list[tuple[str, str]]:
    rows = []
    cursor = 0
    while cursor < len(data):
        end = data.find(b"\0", cursor)
        require(end >= 0, "unterminated mnemonic name")
        name = data[cursor:end]
        cursor = end + 1
        if not name:
            require(not any(data[cursor:]), "nonzero bytes after mnemonic-table terminator")
            break
        end = data.find(b"\0", cursor)
        require(end >= 0, f"unterminated descriptor for {name!r}")
        descriptor = data[cursor:end]
        cursor = end + 1
        rows.append((name.decode("ascii"), descriptor.decode("ascii")))
    return rows


def reachable(instructions: dict[int, tuple[str, tuple[int, ...], int]]) -> set[int]:
    seen = set()
    pending = [0]
    while pending:
        address = pending.pop()
        require(address in instructions, f"control target {address:#x} is not an instruction")
        if address in seen:
            continue
        seen.add(address)
        name, operands, following = instructions[address]
        if name in {"halt", "ret"}:
            successors = ()
        elif name == "jmp":
            successors = (operands[-1],)
        elif name in {"jz", "jnz", "jlt", "jeq", "call"}:
            successors = (operands[-1], following)
        else:
            successors = (following,)
        pending.extend(successors)
    return seen


def main() -> None:
    source_bytes = SOURCE.read_bytes()
    tape = TAPE.read_bytes()
    require(len(source_bytes) == SOURCE_SIZE, "source size changed")
    require(hashlib.sha256(source_bytes).hexdigest() == SOURCE_SHA256, "source identity changed")
    require(len(tape) == TAPE_SIZE, "tape size changed")
    require(hashlib.sha256(tape).hexdigest() == TAPE_SHA256, "tape identity changed")

    source = source_bytes.decode("ascii")
    reconstructed = beta_ref.assemble(source)
    require(reconstructed == tape, "independent Beta relation disagrees with admitted tape")

    items = beta_ref.parse(source)
    counts = Counter(item[0] for item in items)
    require(counts == {"assert": 52, "ins": 253, "dw": 20}, "source-item inventory changed")
    cursor = 0
    for item in items:
        if item[0] == "assert":
            require(item[1] == cursor, f"address assertion disagrees at {cursor:#x}")
        else:
            cursor += beta_ref.size(item)
    require(cursor == len(tape), "source items do not partition the complete tape")

    instructions = decode(tape)
    inventory = Counter(row[0] for row in instructions.values())
    require(inventory == EXPECTED_INVENTORY, "decoded Alpha instruction inventory changed")
    require(len(reachable(instructions)) == len(instructions), "unreachable compiler instruction")
    require(table_rows(tape[CODE_END:]) == EXPECTED_TABLE, "mnemonic table changed")

    initial = [instructions[address] for address in range(0, 0x50, 0x0A)]
    expected_initial = [
        ("imm", (0x8C, 0x80060), 0x0A),
        ("imm", (0x91, 0x100000), 0x14),
        ("imm", (0x92, 0x4100000), 0x1E),
        ("imm", (0x95, 0xFFFFFC), 0x28),
        ("imm", (0x9A, 1), 0x32),
        ("imm", (0x9B, 8), 0x3C),
        ("imm", (0x9C, 9), 0x46),
        ("imm", (0x9D, 0x30), 0x50),
    ]
    require(initial == expected_initial, "memory/profile initialization changed")

    print(
        "Beta root audit: 12,536-byte source -> 52 assertions + 273 emitting items -> "
        "253 reachable Alpha instructions + 160 table bytes -> 1,773-byte tape"
    )


if __name__ == "__main__":
    main()
