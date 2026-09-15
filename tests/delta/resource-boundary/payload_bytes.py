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
# Profile 1 contributes marker28 + the bound support members (bytes 1,464,
# conformance runtime 1,071, adapter 463) + final LF1 = 3,027 bytes; the
# authored identity main contributes 41.
# The unpadded total is 16,762,950. A 14,239-byte tail name adds 14,259;
# one extra name byte crosses the 16,777,212-byte payload provision exactly.
# These are closed-form fixture premises, not a host serializer or usage model.
assert 3027 + 41 + 246 * 67467 + 125 * (3 + 14 * 4 + 141 * 5 + 90 * 6) == 16762950
assert 16762950 + 3 + 14239 + 20 == 16777212


def accepted_fixtures():
    # This receipt digest was observed from the source-owned compilation, not
    # manufactured by a host serializer. Its exact-size execution can use
    # only empty input: receipt plus four framing bytes fills Gamma's request.
    return (
        ("exact payload extent publishes the complete receipt", source(14239),
         172478, "d08f5bd8187c82d4f2917e1f2046a52cbd94904f92430ba68dcac7274ddb4f0f",
         16777212, "51f105126ea6f4d04829c53b38ea7d88dde3520f64d230ca11bf10b505951d5a",
         b"", b""),
    )


def fixtures():
    return (
        ("adjacent payload extent refuses before publication", source(14240),
         172479, "5cd3e92f1425a3cb826a81a8b3555c729ccff3725a84f20d1c03e8234f280867",
         (2, struct.pack(
             "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", 2, 2, 0, 12,
             16777212, 16777212, 16777213,
         ))),
    )


def reconstructed_wide_fixtures():
    fields = b" ".join(f"field{index:05}".encode() for index in range(65535))
    source = (b"(data Wide (Wide" + b" Int" * 65535 + b"))\n"
              b"(def select ((value Wide)) Wide (match value ((Wide "
              + fields + b") (Wide " + fields + b"))))\n"
              b"(def main ((source Bytes)) Bytes source)\n")
    # Closed-form serialization, not a count harvested from compiler output:
    # original payload 4,448,550 (3,027 of it bound support members and the
    # entry LF); 774 helper wrappers; helper-ID digits 2,212;
    # 516 payload captures (8-byte names); 16,909,062 field captures (10 bytes).
    # Each helper adds 16 + 2*name_length + 2*sum(binding_lengths) + 8*arity.
    assert 4448550 + 774 * 20 + 2 * 2212 + 516 * 24 + 16909062 * 28 == 477934574
    return (
        ("full-width reconstruction reaches exact payload refusal", source,
         1704033, "c69598944c34dc0f37187fb67bcf5624b021ac393a8cd8d91f7b967ab84a0945",
         (2, struct.pack(
             "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", 2, 2, 0, 12,
             16777212, 16777212, 477934574,
         ))),
    )
