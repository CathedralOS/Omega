"""The DCOUT outcome row writer shared by the boundary fixture tables."""

import struct


def outcome(tag, code, coordinate, limit=0, requested=0):
    return tag, struct.pack(
        "<8sBBHIQQQ", b"\xffDCOUT\x01\x00", tag, 1, 0, code,
        coordinate, limit, requested,
    )
