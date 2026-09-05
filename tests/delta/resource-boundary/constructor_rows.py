"""Full authored constructor-row boundaries across one or several data owners."""

import struct


def outcome(tag, code, coordinate, limit=0, requested=0):
    return tag, struct.pack(
        "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", tag, 1, 0, code,
        coordinate, limit, requested,
    )


def fixtures():
    # Fixed authored spellings, not a parser or an injected compiler table.
    first = b"  (C00000 Missing)\n"
    constructors = tuple(
        f"  (C{index:05d})\n".encode("ascii") for index in range(1, 65536)
    )
    prefix = b"(data T\n" + first + b"".join(constructors)
    main = b"(def main ((input Bytes)) Bytes input)\n"
    exact = prefix + b")\n" + main
    fresh = prefix + b"  (C65536)\n)\n" + main
    duplicate = prefix + b"  (C00000)\n)\n" + main
    split = (
        b"(data T\n" + first + b"".join(constructors[:32767])
        + b")\n(data U\n" + b"".join(constructors[32767:])
        + b"  (C65536)\n)\n" + main
    )
    duplicate_type = prefix + b")\n(data T (C65536))\n" + main

    # Literal byte coordinates are independent expectations, not compiler output.
    assert exact[18:25] == b"Missing"
    assert fresh[720915:720921] == b"C65536"
    assert duplicate[720915:720921] == b"C00000"
    assert split[720925:720931] == b"C65536"
    assert duplicate_type[720920:720921] == b"T"

    return (
        ("65536 constructors reach declaration resolution", exact,
         720953, "008eae7ba0352c0ec4d6e499e14c2efe33cfad62ee7d840b9b9cb4c56a3df9b5",
         outcome(1, 11, 18)),
        ("65537th fresh constructor refuses before declaration resolution", fresh,
         720964, "4692576840a92f9e54912ce4e3b9e7ad935b56afd136a1e7cc8d04b40b9c1cc2",
         outcome(2, 3, 720915, 65536, 65537)),
        ("duplicate after 65536 constructors retains duplicate diagnosis", duplicate,
         720964, "d3820f5d57b76981d608a31035ca2c82e3ec582778551b056bc591fd5f213f1b",
         outcome(1, 7, 720915)),
        ("constructor rows accumulate across data declarations", split,
         720974, "f208a0895f4926000ef1bb4afd573afc0feb7d5f38039b264c0797f9c9381550",
         outcome(2, 3, 720925, 65536, 65537)),
        ("duplicate type precedes its fresh constructor provision", duplicate_type,
         720971, "96e61c0a7b7ad86802b2b6f7bee31890ed88bf3f3afe88ac27285081aed07ab0",
         outcome(1, 6, 720920)),
    )
