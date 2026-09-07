"""Coordinate concept-owned literal fixtures; never decode submitted records."""

from groups import cases as group_cases
from mutations import cases as mutation_cases
from physical_cases import cases as physical_cases, source_spine
from wire import LIMIT, MAGIC, envelope, example, rejected, words


def cases():
    yield from physical_cases()
    yield from mutation_cases()
    yield from group_cases()

    # Outer failures must be returned unchanged, before examining inner bytes.
    yield "outer_short", b"X" * 23, rejected(23, 1), 2, 60
    valid = envelope(example())
    yield "outer_identity", b"X" + valid[1:], rejected(0, 2), 2, 60
    high = MAGIC + words(0x80000000, 0, 0, 0)
    yield "outer_length_high_bit", high, rejected(11, 3), 2, 60
    missing = MAGIC + words(1, 0, 0, 0)
    yield "outer_extent", missing, rejected(8, 4), 2, 60
    yield "outer_trailing", valid + b"X", rejected(228, 5), 2, 60

    yield source_spine()
    # This tests forwarding, not an 8-MiB inner scan: outer admission stops first.
    oversized = MAGIC + words(LIMIT - 23, 0, 0, 0) + b"\x00" * (LIMIT - 23)
    incomplete = b"\x02" + words(1, LIMIT, LIMIT, LIMIT + 1)
    yield "outer_request_capacity_forwarded", oversized, incomplete, 1, 600
