"""Read an Alpha compiler tape's encoding and optionally check every instruction against plain Alpha."""

import argparse
import hashlib
from pathlib import Path
import struct


# Length in bytes, including opcode, register operands and any 64-bit word.
# See bootstrap/alpha/SEMANTICS.md. Only the jump, conditional branches and
# call carry a code address. An immediate can have any value and stays intact.
WIDTHS = (2, 10, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 9, 10, 10, 11, 11, 2, 2, 9, 1)
BRANCHES = (12, 13, 14, 15, 16, 19)


def inspect(tape, plain=None):
    # Read the four constants that the decoder loads at entry. Match each
    # immediate opcode and its destination register before using the word.
    def immediate(offset, register):
        if tape[offset:offset + 2] != bytes((1, register)) or offset + 10 > len(tape):
            raise ValueError("unrecognized packed Alpha entry")
        return struct.unpack_from("<Q", tape, offset + 2)[0]

    table = immediate(0, 3)
    table_size = immediate(13, 12)
    stream_size = immediate(29, 12)
    base = immediate(42, 1)
    end = immediate(52, 2)
    cursor = table + table_size
    if not (62 <= table <= cursor <= len(tape) <= 16_777_212 and
            table_size % 16 == 0 and table_size <= 240 * 16 and
            cursor + stream_size == len(tape) and 0x28000000 == base < end <= 0x30000000):
        raise ValueError("invalid packed Alpha dimensions")

    entries = []
    for pos in range(table, cursor, 16):
        width = tape[pos]
        if not 1 <= width <= 11:
            raise ValueError("invalid dictionary instruction width")
        entries.append(tape[pos + 1:pos + 1 + width])

    size = branches = 0
    digest = hashlib.sha256()
    while cursor < len(tape):
        token = tape[cursor]
        cursor += 1
        if 1 <= token <= 11:
            instruction = tape[cursor:cursor + token]
            cursor += token
        elif 16 <= token < 16 + len(entries):
            instruction = entries[token - 16]
        else:
            raise ValueError("invalid instruction token")
        if not instruction or instruction[0] >= len(WIDTHS) or len(instruction) != WIDTHS[instruction[0]]:
            raise ValueError(f"invalid instruction at expanded byte {size}")
        if instruction[0] in BRANCHES:
            target = int.from_bytes(instruction[-8:], "little")
            if not base <= target <= end:
                raise ValueError(f"unrelocated branch at expanded byte {size}")
            instruction = instruction[:-8] + struct.pack("<Q", target - base)
            branches += 1
        if plain is not None and instruction != plain[size:size + len(instruction)]:
            raise ValueError(f"instruction differs from plain Alpha at byte {size}")
        size += len(instruction)
        digest.update(instruction)
    if cursor != len(tape) or size != end - base or (plain is not None and size != len(plain)):
        raise ValueError("instruction stream length differs")
    return size, branches, digest.hexdigest()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tape", type=Path)
    parser.add_argument("plain", nargs="?", type=Path, help="ordinary instructions assembled at address zero")
    args = parser.parse_args()
    size, branches, digest = inspect(args.tape.read_bytes(), args.plain.read_bytes() if args.plain else None)
    print(f"{size} plain Alpha bytes, {branches} rebased branches and calls, sha256={digest}")
