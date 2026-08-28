#!/usr/bin/env python3
"""Focused exactness and resource tests for packed Gamma input transport."""

from __future__ import annotations

import importlib.util
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("encode-gamma-input.py")
SPEC = importlib.util.spec_from_file_location("encode_gamma_input", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
ENCODER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ENCODER)


def unpack(words: list[int], length: int) -> bytes:
    result = bytearray()
    for word in words:
        result.extend(word.to_bytes(4, "little"))
    return bytes(result[:length])


def check(raw: bytes, *, legacy: bool = True) -> None:
    words = ENCODER.pack_words(raw)
    assert unpack(words, len(raw)) == raw
    carrier = ENCODER.encode(raw)
    assert carrier.startswith(f"(Chunks {len(raw)} ")
    assert carrier.endswith(")")
    assert carrier.count("(Node ") <= len(words) * ENCODER.TREE_DEPTH
    assert ENCODER.inject(b"before STDIN after", raw) == (
        b"before " + carrier.encode("ascii") + b" after"
    )
    if legacy:
        assert ENCODER.inject(b"STDIN", raw, legacy_cons=True) == ENCODER.encode_cons(
            raw
        ).encode("ascii")


def main() -> None:
    for raw in (b"", b"\0", b"\0\xff", b"abc", b"abcd", b"abcde"):
        check(raw)
    check(bytes(range(256)))
    maximum = bytes(index & 0xFF for index in range(ENCODER.MAX_INPUT_BYTES))
    check(maximum, legacy=False)
    try:
        ENCODER.encode(maximum + b"x")
    except ENCODER.InputTooLarge:
        pass
    else:
        raise AssertionError("524,288-byte-plus-one input was admitted")
    for malformed in (b"no placeholder", b"STDIN and STDIN"):
        try:
            ENCODER.inject(malformed, b"")
        except ValueError:
            pass
        else:
            raise AssertionError("non-singleton placeholder was admitted")
    print("packed Gamma input: exact 0..5/NUL/all-byte/maximal carriers and +1 refusal passed")


if __name__ == "__main__":
    main()
