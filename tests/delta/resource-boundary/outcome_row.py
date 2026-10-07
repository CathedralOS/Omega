"""The DCOUT row writers shared by the boundary fixture tables."""

import struct


def outcome(tag, code, coordinate, limit=0, requested=0):
    return tag, struct.pack(
        "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", tag, 1, 0, code,
        coordinate, limit, requested,
    )


def rejection(code, coordinate):
    return 1, struct.pack(
        "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", 1, 1, 0, code,
        coordinate, 0, 0,
    )
