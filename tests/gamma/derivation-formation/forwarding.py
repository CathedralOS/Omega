"""Physical admission remains ahead of every theory semantic provision."""

from theory_wire import certificate, envelope, failure, proposition, record, theory
from wire import changed_word


def cases():
    def rejected(name, request, expected):
        return name, request, expected, 2, 60

    yield rejected("short_outer_header", b"", failure(0, 1))
    valid = envelope((theory((record(1, 0),)), proposition(), certificate()))
    yield rejected("outer_identity", b"X" + valid[1:], failure(0, 2))
    yield rejected("outer_trailing_input", valid + b"X", failure(len(valid), 5))
    yield rejected("outer_extent_before_zero_sort", changed_word(valid, 28, 0)[:-1], failure(16, 4))
    oversized = envelope((theory(sorts=65537), proposition(), certificate()))
    yield rejected("layout_magic_before_sort_resource", oversized[:24] + b"X" + oversized[25:], failure(24, 6))
    yield rejected("later_layout_before_sort_resource", oversized[:-1] + b"\x80", failure(len(oversized) - 1, 6))
    constructors = (record(0, 148, *([1] * 148)),) + (record(1, 0),) * 9
    work = envelope((theory(constructors, sorts=52753), proposition(), certificate()))
    yield rejected("layout_high_bit_before_work_resource", changed_word(work, 40, 0x80000000), failure(43, 6))
    invalid = changed_word(valid, 40, 0)
    yield rejected("later_layout_before_signature", invalid[:-1] + b"\x80", failure(len(invalid) - 1, 6))
