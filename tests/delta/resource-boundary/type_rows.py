"""Full authored type-row boundaries, including the two built-in type rows."""

import struct


def outcome(tag, code, coordinate, limit=0, requested=0):
    return tag, struct.pack(
        "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", tag, 1, 0, code,
        coordinate, limit, requested,
    )


def fixtures():
    # Int and Bytes occupy two rows; these 65,534 nominal types fill the rest.
    prefix = b"(data T00000 (C00000 Missing))\n" + b"".join(
        f"(data T{index:05d} (C{index:05d}))\n".encode("ascii")
        for index in range(1, 65534)
    )
    main = b"(def main ((input Bytes)) Bytes input)\n"
    exact = prefix + main
    fresh = prefix + b"(data T65534 (C65534))\n" + main
    duplicate = prefix + b"(data T00000 (C65534))\n" + main
    duplicate_constructor = prefix + b"(data T65534 (C00000))\n" + main

    # Coordinates name literal source spans, independent of compiler output.
    assert exact[21:28] == b"Missing"
    assert fresh[1507296:1507302] == b"T65534"
    assert duplicate[1507296:1507302] == b"T00000"
    assert duplicate_constructor[1507296:1507302] == b"T65534"

    return (
        ("65536 total types reach declaration resolution", exact,
         1507329, "14491e164eb116460baf83103e543d8c17626609b227d9bcf97140a1b0dd59fa",
         outcome(1, 11, 21)),
        ("65537th total type refuses before declaration resolution", fresh,
         1507352, "16e32aa1ce51e284331b03121f5ba8374f5c8accbe5dfcd26d8d89acb08267e3",
         outcome(2, 2, 1507296, 65536, 65537)),
        ("duplicate after 65536 total types retains duplicate diagnosis", duplicate,
         1507352, "ce9b001814e8523bb6d74328e5e77f00f47234182083b3cb2442c08bfa33dfa4",
         outcome(1, 6, 1507296)),
        ("fresh type refuses before its duplicate constructor lookup", duplicate_constructor,
         1507352, "9595c852bae72a4a5f6f5d5b874610822bd6a013415af8e6286ad9a829c35c02",
         outcome(2, 2, 1507296, 65536, 65537)),
    )
