#!/usr/bin/env python3
"""Encode exact Delta input as a bounded ordinary-Gamma value.

The carrier is ``(Chunks byte_length tree)``.  A fixed-depth-17 binary tree
contains 131,072 little-endian u32 leaves, enough for Delta's 524,288-byte
sealed-input ceiling.  Four bytes share one immediate Gamma integer.  Leaves
use bit-reversed physical indexes so lookup consumes one low index bit per tree
level; no large span arithmetic enters the Gamma meaning or its claim encoder.
Entirely zero subtrees collapse to ``ZeroTree``; the exact byte length makes
word padding unobservable.

This is invocation transport, not Gamma syntax.  ``omega2gamma.beta`` also
retains its historical ``Cons`` input interpretation for small fixtures.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


MAX_INPUT_BYTES = 524_288
TREE_DEPTH = 17
WORD_CAPACITY = 1 << TREE_DEPTH
PLACEHOLDER = b"STDIN"


class InputTooLarge(ValueError):
    """The exact input crosses Delta's sealed-input resource ceiling."""


def pack_words(raw: bytes) -> list[int]:
    """Return little-endian u32 words, retaining a partial final word."""

    if len(raw) > MAX_INPUT_BYTES:
        raise InputTooLarge(
            f"Gamma input is {len(raw)} bytes; ceiling is {MAX_INPUT_BYTES}"
        )
    words: list[int] = []
    for offset in range(0, len(raw), 4):
        word = 0
        for lane, byte in enumerate(raw[offset : offset + 4]):
            word |= byte << (lane * 8)
        words.append(word)
    return words


def encode(raw: bytes) -> str:
    """Return a depth-bounded Gamma constructor expression for exact *raw*."""

    words = pack_words(raw)
    level = ["ZeroTree"] * WORD_CAPACITY
    for logical_index, word in enumerate(words):
        physical_index = int(f"{logical_index:0{TREE_DEPTH}b}"[::-1], 2)
        level[physical_index] = str(word)
    for _ in range(TREE_DEPTH):
        parent: list[str] = []
        for index in range(0, len(level), 2):
            left = level[index]
            right = level[index + 1]
            if left == "ZeroTree" and right == "ZeroTree":
                parent.append("ZeroTree")
            else:
                parent.append(f"(Node {left} {right})")
        level = parent
    assert len(level) == 1
    return f"(Chunks {len(raw)} {level[0]})"


def encode_cons(raw: bytes) -> str:
    """Historical small-input carrier, retained only for equivalence tests."""

    if len(raw) > MAX_INPUT_BYTES:
        raise InputTooLarge(
            f"Gamma input is {len(raw)} bytes; ceiling is {MAX_INPUT_BYTES}"
        )
    # Build once instead of repeatedly prepending an ever-growing string. The
    # compatibility form remains linear-depth, but constructing bounded proof
    # fixtures must not itself be quadratic.
    return "".join(f"(Cons {byte} " for byte in raw) + "Nil" + ")" * len(raw)


def inject(template: bytes, raw: bytes, *, legacy_cons: bool = False) -> bytes:
    """Close one omega2gamma template with the exact packed carrier."""

    count = template.count(PLACEHOLDER)
    if count != 1:
        raise ValueError(f"expected one STDIN placeholder, found {count}")
    carrier = encode_cons(raw) if legacy_cons else encode(raw)
    return template.replace(PLACEHOLDER, carrier.encode("ascii"))


def read_bytes(path: str) -> bytes:
    return sys.stdin.buffer.read() if path == "-" else Path(path).read_bytes()


def write_bytes(path: str, payload: bytes) -> None:
    if path == "-":
        sys.stdout.buffer.write(payload)
    else:
        Path(path).write_bytes(payload)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    encode_command = commands.add_parser("encode", help="encode INPUT as a carrier")
    encode_command.add_argument("input")
    encode_command.add_argument("output", nargs="?", default="-")
    inject_command = commands.add_parser("inject", help="replace STDIN in TEMPLATE")
    inject_command.add_argument("template")
    inject_command.add_argument("input")
    inject_command.add_argument("output", nargs="?", default="-")
    legacy_command = commands.add_parser(
        "inject-cons", help="test-only historical Cons substitution"
    )
    legacy_command.add_argument("template")
    legacy_command.add_argument("input")
    legacy_command.add_argument("output", nargs="?", default="-")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    try:
        raw = read_bytes(args.input)
        if args.command == "encode":
            payload = encode(raw).encode("ascii")
        else:
            payload = inject(
                read_bytes(args.template), raw, legacy_cons=args.command == "inject-cons"
            )
        write_bytes(args.output, payload)
    except InputTooLarge as error:
        print(f"encode-gamma-input: {error}", file=sys.stderr)
        return 252
    except (OSError, ValueError) as error:
        print(f"encode-gamma-input: {error}", file=sys.stderr)
        return 251
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
