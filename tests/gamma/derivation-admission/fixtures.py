"""Physical request vectors and authored expectations, not a decoder model."""

import struct


MAGIC = b"GDREQ\x01\x00\x00"
LIMIT = 8388608


def envelope(theory=b"", proposition=b"", certificate=b""):
    return header(len(theory), len(proposition), len(certificate)) + theory + proposition + certificate


def header(theory, proposition, certificate, reserved=0):
    return MAGIC + struct.pack("<IIII", theory, proposition, certificate, reserved)


def framed(theory_end, proposition_end, certificate_end):
    return b"\x00" + struct.pack("<III", theory_end, proposition_end, certificate_end)


def rejected(code, coordinate):
    return b"\x01" + struct.pack("<IIII", code, coordinate, 0, 0)


def cases():
    empty = envelope()
    for extent in range(24):
        yield f"short_header_{extent}", empty[:extent], rejected(1, extent), 2
    yield "short_corrupt_header", b"X" * 23, rejected(1, 23), 2

    for offset in (*range(8), *range(20, 24)):
        altered = bytearray(empty)
        altered[offset] ^= 1
        yield f"identity_or_reserved_{offset}", bytes(altered), rejected(2, offset), 2

    for offset in (11, 15, 19):
        altered = bytearray(empty)
        altered[offset] = 128
        yield f"length_high_bit_{offset}", bytes(altered), rejected(3, offset), 2
    yield "all_length_high_bits", header(0x80000000, 0x80000000, 0x80000000), rejected(3, 11), 2
    yield "reserved_before_length", header(0xFFFFFFFF, 0, 0, 1), rejected(2, 20), 2
    yield "identity_before_reserved", b"X" + header(0, 0, 0, 1)[1:], rejected(2, 0), 2
    yield "all_high_bits_before_extent", header(0x7FFFFFFF, 0, 0x80000000), rejected(3, 19), 2

    yield "missing_theory", header(1, 0, 0), rejected(4, 8), 2
    yield "missing_proposition", header(1, 1, 0) + b"T", rejected(4, 12), 2
    yield "missing_certificate", header(1, 1, 1) + b"TP", rejected(4, 16), 2
    yield "maximum_theory_claim", header(0x7FFFFFFF, 0, 0), rejected(4, 8), 2
    yield "maximum_proposition_claim", header(1, 0x7FFFFFFF, 0) + b"T", rejected(4, 12), 2
    yield "maximum_certificate_claim", header(1, 1, 0x7FFFFFFF) + b"TP", rejected(4, 16), 2
    yield "all_maximum_claims", header(0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF), rejected(4, 8), 2
    yield "nonzero_fourth_theory_byte", header(0x01000000, 0, 0), rejected(4, 8), 2
    yield "nonzero_fourth_proposition_byte", header(0, 0x01000000, 0), rejected(4, 12), 2
    yield "nonzero_fourth_certificate_byte", header(0, 0, 0x01000000), rejected(4, 16), 2
    yield "fourth_theory_byte_with_fitting_low_bytes", header(0x01000001, 0, 0) + b"T", rejected(4, 8), 2
    yield "fourth_proposition_byte_with_fitting_low_bytes", header(0, 0x01000001, 0) + b"P", rejected(4, 12), 2
    yield "fourth_certificate_byte_with_fitting_low_bytes", header(0, 0, 0x01000001) + b"C", rejected(4, 16), 2
    yield "trailing_empty", empty + b"X", rejected(5, 24), 2
    yield "trailing_nonempty", envelope(b"T", b"P", b"CCCC") + b"X", rejected(5, 30), 2

    yield "framed_empty", empty, framed(24, 24, 24), 2
    yield "framed_theory_only", envelope(b"abc"), framed(27, 27, 27), 2
    yield "framed_proposition_only", envelope(proposition=b"abc"), framed(24, 27, 27), 2
    yield "framed_certificate_only", envelope(certificate=b"abc"), framed(24, 24, 27), 2
    # Deliberately opaque bytes: framing does not judge any inner language.
    yield "opaque_sections_not_proof_acceptance", envelope(b"\x00\xffA", b"PQ", b"\xff\x00XY"), framed(27, 29, 33), 2

    # The largest requests are constructed lazily and each is observed once.
    yield "exact_theory_capacity", envelope(b"\x00" * (LIMIT - 24)), framed(LIMIT, LIMIT, LIMIT), 1
    yield "exact_certificate_capacity", envelope(certificate=b"\xff" * (LIMIT - 24)), framed(24, 24, LIMIT), 1
    incomplete = b"\x02" + struct.pack("<IIII", 1, LIMIT, LIMIT, LIMIT + 1)
    yield "adjacent_request_capacity", envelope(certificate=b"\x00" * (LIMIT - 23)), incomplete, 1
    yield "oversized_trailing_before_capacity", empty + b"\x00" * (LIMIT - 23), rejected(5, 24), 1
    yield "oversized_identity_before_capacity", b"X" + header(LIMIT - 23, 0, 0)[1:] + b"\x00" * (LIMIT - 23), rejected(2, 0), 1
