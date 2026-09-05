"""Full authored syntax allocations; no host parser or injected usage state."""

import struct


def rejection(code, coordinate):
    return 1, struct.pack(
        "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", 1, 1, 0, code,
        coordinate, 0, 0,
    )


def incomplete(coordinate, requested):
    return 2, struct.pack(
        "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", 2, 1, 0, 7,
        coordinate, 114294752, requested,
    )


def fixtures():
    # A balanced program with L lists and A atoms allocates 8L + 5A + 1
    # parser pairs. Each pair occupies 40 bytes. The selected byte limit has
    # 32 unused tail bytes beyond its 2,857,368 complete pairs.
    aligned = b"()" * 357169 + b"a a a"
    extra_list = aligned + b"()"
    definition = b"(def f () Int 0)"
    below_grammar = definition * 59528
    above_grammar = definition * 59529

    # These equalities derive from the authored construction, not compiler
    # output. They do not replace the source compiler's allocation ledger.
    assert (8 * 357169 + 5 * 3 + 1) * 40 == 114294720
    assert (8 * 357170 + 5 * 3 + 1) * 40 == 114295040
    assert (952457 * 3) * 40 == 114294840
    assert ((714342 + 1) * 4) * 40 == 114294880
    assert (3 + 4 * 571473 + 571473 + 4) * 40 == 114294880
    assert (48 * 59528 + 1) * 40 == 114293800
    assert (36 * 59529 + 1 + 12 * 59527) * 40 == 114294760
    assert 59526 * len(definition) == 952416

    return (
        ("maximum aligned parser storage reaches first grammar shape", aligned,
         714343, "ba5b3b268d60810d5d6f6bc3172b1912629a35e7ab6a39236ba0a3fc79a0e9df",
         rejection(4, 1)),
        ("additional list refuses complete EOF spine and root group", extra_list,
         714345, "0892427c5a2b693ac1c833559b917a4a307e14015897dd2e3bd6841cda4b5024",
         incomplete(714345, 114295040)),
        ("opening frame refuses before later unclosed source", b"(" * 952457,
         952457, "818aab87587cf2d454c2dfb5fc0b5a512c59a91882a8c9cfe771ddedc4ee2870",
         incomplete(952456, 114294840)),
        ("atom node and reversed spine refuse together", b"a " * 714342 + b"a",
         1428685, "46e22be98a623508389823b61cef13dd79b81565b884596afa327af4f1b226bf",
         incomplete(1428684, 114294880)),
        ("closing list refuses its complete ordered child spine",
         b"(" + b"a " * 571473 + b")",
         1142948, "5731ee0bd32fd36dd374de43fb606edf184f87ba8781ef5a318691b2e69443bf",
         incomplete(0, 114294880)),
        ("cumulative grammar below limit reaches duplicate census", below_grammar,
         952448, "3f51b663af6446af8f713faef055870b7d8e74a1ec48b48c0201a6ad793ab02b",
         rejection(8, 21)),
        ("grammar frames accumulate across declarations", above_grammar,
         952464, "9044f155ac3018ff27c0eeb71715f9e86a1d59c3ac72becc54fe55cca7d7de02",
         incomplete(952416, 114294760)),
        ("forbidden source byte precedes syntax storage", extra_list + b"\x00",
         714346, "aadca15a77a0a49fa133ce6aa8bec9704906f66c43bc9a95ecaa363349aee686",
         rejection(3, 714345)),
        ("unmatched close precedes pending EOF provision", extra_list + b")",
         714346, "7eb78f780a6b51c014bd5ecabef665d9ffb360fe29801c601ae3d8ae3d658188",
         rejection(4, 714345)),
        ("first malformed declaration precedes later grammar provision",
         b"()" + above_grammar,
         952466, "e80ca469847e922c37a35b5ae7a49d9687c859dd45a20c410f13ab7b1571b3fb",
         rejection(4, 1)),
    )
