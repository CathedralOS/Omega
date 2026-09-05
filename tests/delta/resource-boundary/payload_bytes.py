"""Authored payload boundaries; no host-generated Gamma receipt."""

import struct


def source(padding):
    data = b"(data T (C" + b" Int" * 120 + b"))\n"
    binders = b" ".join(f"p{index:03d}".encode() for index in range(120))
    definitions = b"".join(
        b"(def " + f"f{index:03d}".encode()
        + b" ((x T)) Int (match x ((C " + binders + b") 0)))\n"
        for index in range(246)
    )
    # This name is after every match, so padding cannot change the generated
    # match-coordinate spellings. Its definition adds name-length + 20 bytes.
    tail = b"(def pad" + b"x" * padding + b" () Int 0)\n"
    return data + definitions + tail + b"(def main ((source Bytes)) Bytes source)\n"


# Data occupies 493 source bytes; each match definition occupies 641. Match
# starts are 515 + 641*i, with decimal-length counts {3:1, 4:14, 5:141, 6:90}.
# For d coordinate digits, all payload projections total 65,452 + 120*d bytes.
# Binder lets, terminal 0, three wrappers, and the function envelope/LF give
# 67,467 + 125*d emitted bytes per definition. Height is 242: no helper split.
# Profile 1 contributes marker28 + Bytes660 + Conformance526 + adapter154
# + final LF1 = 1,369 bytes; the authored identity main contributes 41.
# The unpadded total is 16,761,292. A 15,900-byte tail name adds 15,920;
# one extra name byte crosses the 16,777,212-byte payload provision exactly.
# These are closed-form fixture premises, not a host serializer or usage model.
assert 1369 + 41 + 246 * 67467 + 125 * (3 + 14 * 4 + 141 * 5 + 90 * 6) == 16761292
assert 16761292 + 3 + 15897 + 20 == 16777212


def accepted_fixtures():
    # This receipt digest was observed from the source-owned compilation, not
    # manufactured by a host serializer. Its exact-size execution can use
    # only empty input: receipt plus four framing bytes fills Gamma's request.
    return (
        ("exact payload extent publishes the complete receipt", source(15897),
         174136, "59c34931c6cfc0ba7b02483b5422857a3b7ccc4a3ec4749e9fca1fa631f565ed",
         16777212, "d20cd2be86566d9d5dd78410eb0ef9fb691fef795546f56de9394313e1514f21"),
    )


def fixtures():
    return (
        ("adjacent payload extent refuses before publication", source(15898),
         174137, "aa958dbee153d1dd7b89842fc217cfa6f48ab44a9761a048fef2e63a2ace88d2",
         (2, struct.pack(
             "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", 2, 2, 0, 12,
             16777212, 16777212, 16777213,
         ))),
    )
