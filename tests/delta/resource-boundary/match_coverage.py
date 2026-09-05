"""Full per-match coverage sets, diagnostic precedence, and nested isolation."""

import struct


def rejection(code, coordinate):
    return 1, struct.pack(
        "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", 1, 1, 0, code,
        coordinate, 0, 0,
    )


def fixtures():
    names = tuple(f"C{index:05d}".encode("ascii") for index in range(65536))
    declarations = b"(data T " + b" ".join(
        b"(" + name + b")" for name in names
    ) + b")\n"
    arms = b" ".join(b"(" + name + b" 0)" for name in names)
    main = b"(def main () Int "
    body = b"(match C00000 " + arms + b")"
    exact = declarations + main + body + b")\n"
    duplicate = declarations + main + b"(match C00000 " + arms + b" (C00000 0)))\n"
    nested = declarations + main + b"(match C00000 (C00000 " + body + b")))\n"
    reversed_arms = declarations + main + b"(match C00000 " + b" ".join(
        b"(" + name + b" 0)" for name in reversed(names)
    ) + b"))\n"

    # Fixed authored spans, not coordinates obtained from compiler output.
    assert exact[589838:589842] == b"main"
    assert reversed_arms[589838:589842] == b"main"
    assert duplicate[1310761:1310767] == b"C00000"
    assert nested[589850:589872] == b"(match C00000 (C00000 "
    assert nested[589872:589879] == b"(match "

    return (
        ("65536 distinct match arms reach entry schema checking", exact,
         1310762, "ee5810bc87df71fb3cda85a8a7ccc2344f50b6a3072792df10bf17df0ee789d1",
         rejection(20, 589838)),
        ("duplicate after 65536 match arms retains duplicate diagnosis", duplicate,
         1310773, "61897a1762019dadbf2d14751f7ec0a7377ff9413e94f2ae1eaa7257cc3fbf5a",
         rejection(17, 1310761)),
        ("nested full coverage does not complete the outer match", nested,
         1310786, "a955aa8a57e201611b0829e2ce90d13f08a028a9418d7cde8a453bad018fd74c",
         rejection(18, 589850)),
        ("65536 reversed match arms reach entry schema checking", reversed_arms,
         1310762, "49d1bcae8f662faa44a9f7cc4e00da68ce66f243c66468dfb62b1fb56e5bb69f",
         rejection(20, 589838)),
    )
