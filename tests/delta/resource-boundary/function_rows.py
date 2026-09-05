"""Full authored function-row boundaries; no parser or injected compiler state."""

import struct


def fixtures():
    declarations = b"(data Flag (Off) (On))\n"
    first = declarations + b"(def f00000 () Missing 0)\n"
    exact = first + b"".join(
        f"(def f{index:05d} () Int 0)\n".encode("ascii")
        for index in range(1, 32768)
    )
    assert len(declarations) + len(b"(def f00000 () ") == 38
    assert len(exact) + len(b"(def ") == 720928

    def outcome(tag, code, coordinate, limit=0, requested=0):
        return tag, struct.pack(
            "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", tag, 1, 0, code,
            coordinate, limit, requested,
        )

    return (
        ("32768 distinct functions reach declaration resolution", exact,
         720923, "f281abc5132fafa0be7a2cee9f682e7490783cce01b4f2f96fa194a85aa68b4f",
         outcome(1, 11, 38)),
        ("32769th distinct function refuses before declaration resolution",
         exact + b"(def f32768 () Int 0)\n",
         720945, "be015692ff28c06229b17e49b5682ee17608a4220a028a4d9abfd1805742d07a",
         outcome(2, 4, 720928, 32768, 32769)),
        ("duplicate after 32768 functions retains duplicate diagnosis",
         exact + b"(def f00000 () Int 0)\n",
         720945, "f3f975cc7460a9d8f68dc8475bb1cd6ad627182963e61d8133528d07c2d88d31",
         outcome(1, 8, 720928)),
    )
