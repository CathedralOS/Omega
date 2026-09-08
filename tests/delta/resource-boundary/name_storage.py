"""One long ordinary identifier, below the source and declaration provisions."""


def accepted_fixtures():
    source = (
        b"(def " + b"a" * 4_000_000
        + b" () Int 0)\n(def main ((x Bytes)) Bytes x)\n"
    )
    return ((
        "four-million-byte function name compiles and executes",
        source, 4_000_047,
        "d130918a4a0e50fa0f80161d7cd4862762c74fa7a9f35c14d04891d1f97f2ee0",
        4_001_420,
        "c358648656387d53a38e09c2777309bd6b438d4e00f587d16749279403cb1280",
        b"\x00A\x80\xff", b"\x00A\x80\xff",
    ),)
